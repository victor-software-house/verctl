//! `release` drafts a Release with no tag, uploads its assets, publishes to
//! the registries, then publishes the draft, which creates the tag. `status`
//! and `release` see each state through the binary.

mod common;

use std::path::Path;
use std::process::{Command, Output};
use std::{env, fs};

use indoc::{formatdoc, indoc};
use tempfile::TempDir;
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CONFIG: &str = indoc! {"
    packages:
      - name: demo
        path: Cargo.toml
        publisher: none
    assets:
      linux-x64: {}
"};

fn manifest(version: &str) -> String {
    format!("[package]\nname = \"demo\"\nversion = \"{version}\"\n")
}

/// A repository whose first commit sets `demo` to 1.0.0 and whose second sets
/// 1.1.0, with `origin/main` at the second. Returns both commits.
fn two_versions(root: &Path) -> (String, String) {
    common::write_config(root, CONFIG);
    fs::write(root.join("Cargo.toml"), manifest("1.0.0")).unwrap();
    fs::write(
        root.join("CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## demo 1.0.0

            - First.
        "},
    )
    .unwrap();
    common::init(root);
    let first = common::commit_all(root, "demo 1.0.0");
    fs::write(root.join("Cargo.toml"), manifest("1.1.0")).unwrap();
    fs::write(
        root.join("CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## demo 1.1.0

            - Second.

            ## demo 1.0.0

            - First.
        "},
    )
    .unwrap();
    let second = common::commit_all(root, "demo 1.1.0");
    common::git(
        root,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/demo.git",
        ],
    );
    common::git(root, &["update-ref", "refs/remotes/origin/main", &second]);
    (first, second)
}

fn release_json(tag: &str, draft: bool, assets: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "id": 1,
        "html_url": format!("https://github.com/acme/demo/releases/tag/{tag}"),
        "tag_name": tag,
        "draft": draft,
        "immutable": false,
        "assets": assets
            .iter()
            .map(|name| serde_json::json!({ "name": name, "state": "uploaded" }))
            .collect::<Vec<_>>()
    })
}

async fn tag_at(server: &MockServer, tag: &str, sha: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{tag}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": format!("refs/tags/{tag}"),
            "node_id": "R",
            "url": format!("https://api.github.com/repos/acme/demo/git/refs/tags/{tag}"),
            "object": {
                "type": "commit",
                "sha": sha,
                "url": format!("https://api.github.com/repos/acme/demo/git/commits/{sha}")
            }
        })))
        .mount(server)
        .await;
}

/// GitHub as the two versions leave it: 1.1.0 published with its tarball, and
/// 1.0.0 a draft with none, which only the list shows.
async fn github_with_an_old_draft(first: &str, second: &str) -> MockServer {
    let server = MockServer::start().await;
    tag_at(&server, "v1.1.0", second).await;
    tag_at(&server, "v1.0.0", first).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/tags/v1.1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(release_json(
            "v1.1.0",
            false,
            &["demo_1.1.0_linux_x64.tar.gz"],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/tags/v1.0.0"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "message": "Not Found",
            "documentation_url": "https://docs.github.com"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            release_json("v1.1.0", false, &["demo_1.1.0_linux_x64.tar.gz"]),
            release_json("v1.0.0", true, &[])
        ])))
        .mount(&server)
        .await;
    server
}

/// verctl with no host `CI` or `GITHUB_*` variable, so each run sees only
/// the GitHub the test sets. On a pull request CI sets `GITHUB_BASE_REF`,
/// and the release proof would then fetch that branch from the fixture's
/// fake origin.
fn off_actions() -> Command {
    let mut command = common::verctl();
    for (key, _) in env::vars_os() {
        if key
            .to_str()
            .is_some_and(|key| key == "CI" || key.starts_with("GITHUB_"))
        {
            command.env_remove(key);
        }
    }
    command
}

fn run(root: &Path, server: &MockServer, args: &[&str]) -> Output {
    off_actions()
        .current_dir(root)
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_API_URL", server.uri())
        .args(args)
        .output()
        .unwrap()
}

/// GitHub as HEAD's version leaves it: 1.1.0 a draft with no tarball, which
/// only the list shows. The tag exists.
async fn github_with_a_draft_at_head(second: &str) -> MockServer {
    let server = MockServer::start().await;
    tag_at(&server, "v1.1.0", second).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/tags/v1.1.0"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "message": "Not Found",
            "documentation_url": "https://docs.github.com"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([release_json(
                "v1.1.0",
                true,
                &[]
            )])),
        )
        .mount(&server)
        .await;
    server
}

fn plan(root: &Path, server: &MockServer) -> String {
    let out = root.join("github-output");
    let output = run(
        root,
        server,
        &["status", "--github-output", out.to_str().unwrap()],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(&out).unwrap()
}

/// A draft at a version HEAD does not hold stays unreleased.
/// HEAD's own Release is complete, so the plan is empty and names nothing of
/// the older commit.
#[tokio::test]
async fn a_draft_at_a_version_head_no_longer_holds_is_not_planned() {
    let root = TempDir::new().unwrap();
    let (first, second) = two_versions(root.path());
    let server = github_with_an_old_draft(&first, &second).await;
    let written = plan(root.path(), &server);
    assert!(written.contains("commits=[]"), "{written}");
    assert!(written.contains("pending=false"), "{written}");
    assert!(written.contains(r#"assets={"include":[]}"#), "{written}");
    assert!(written.lines().any(|line| line == "declared="), "{written}");
}

/// A draft at HEAD's version is planned at HEAD: the plan lists HEAD, the
/// asset job the draft lacks, and the assets it expects as comma-separated
/// file names.
#[tokio::test]
async fn a_draft_at_head_is_planned_at_head_with_its_assets() {
    let root = TempDir::new().unwrap();
    let (_, second) = two_versions(root.path());
    let server = github_with_a_draft_at_head(&second).await;
    let written = plan(root.path(), &server);
    assert!(
        written.contains(&format!(r#"commits=["{second}"]"#)),
        "{written}"
    );
    assert!(written.contains("pending=true"), "{written}");
    assert!(
        written.contains(&format!(
            r#"assets={{"include":[{{"commit":"{second}","id":"linux-x64","labels":["ubuntu-26.04"],"tag":"v1.1.0","asset":"demo_1.1.0_linux_x64.tar.gz"}}]}}"#
        )),
        "{written}"
    );
    assert!(
        written
            .lines()
            .any(|line| line == "declared=demo_1.1.0_linux_x64.tar.gz"),
        "{written}"
    );
}

/// `release` at a commit whose Release is finished does nothing, even though
/// an older version's draft is still open. A finalize leg runs there after
/// another leg finished its Release.
#[tokio::test]
async fn release_at_a_finished_commit_reports_nothing_to_release() {
    let root = TempDir::new().unwrap();
    let (first, second) = two_versions(root.path());
    let server = github_with_an_old_draft(&first, &second).await;
    let output = run(root.path(), &server, &["release", "--color", "never"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nothing to release"), "{stdout}");
}

/// `--releases-only` starts no publisher, so a package the registry does not
/// list stops the run before any upload.
#[tokio::test]
async fn releases_only_refuses_a_package_the_registry_lacks() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml
                query: ["true"]
        "#},
    );
    fs::write(root.path().join("Cargo.toml"), manifest("0.0.1")).unwrap();
    fs::write(
        root.path().join("CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## demo 0.0.1

            - First.
        "},
    )
    .unwrap();
    common::on_default_branch(root.path());
    let server = MockServer::start().await;
    for missing in [
        "/repos/acme/demo/git/ref/tags/v0.0.1",
        "/repos/acme/demo/releases/tags/v0.0.1",
    ] {
        Mock::given(method("GET"))
            .and(path(missing))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found",
                "documentation_url": "https://docs.github.com"
            })))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    let output = run(
        root.path(),
        &server,
        &["release", "--releases-only", "--color", "never"],
    );
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let said = format!("{stderr}{stdout}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        said.contains("--releases-only starts no publisher"),
        "{said}"
    );
}

/// How a test package's upload behaves.
#[derive(Clone, Copy)]
enum Upload {
    /// The registry lists the version from the start, so no upload runs.
    Listed,
    /// The upload reaches the mock, and the registry lists the version after.
    Succeeds,
    /// The upload reaches the mock, then fails with crates.io's 429.
    RateLimited,
}

/// What crates.io prints when it refuses an upload for its rate limit.
const RATE_LIMITED: &str = "the remote server responded with an error (status 429 Too Many Requests): Please try again after Fri, 13 Nov 2026 19:00:00 GMT or email help@crates.io";

/// The tag every crate of the order tests shares.
const TAG: &str = "v0.4.0";

/// The tarball the order tests declare.
const TARBALL: &str = "core_0.4.0_linux_x64.tar.gz";

/// Write `body` as the shell script `name` in `root`, and return the argv
/// that runs it through `sh`, so a test never executes a file it wrote a moment
/// before.
fn script(root: &Path, name: &str, body: &str) -> String {
    let path = root.join(name);
    fs::write(&path, body).unwrap();
    format!(r#"["sh", "{}"]"#, path.display())
}

/// One package of [`crates`]: its manifest and changelog at `0.4.0`, and its
/// entry under `packages`, whose upload `curl -fsS <uri>/__upload/{name}` the
/// mock records and whose `query` lists the version once that upload ran.
fn crate_entry(root: &Path, uri: &str, name: &str, upload: Upload) -> String {
    let marker = root.join(format!("{name}.up"));
    let marker = marker.display();
    let upload_body = match upload {
        Upload::Listed | Upload::Succeeds => formatdoc! {r#"
            curl -fsS "{uri}/__upload/{name}" > /dev/null
            touch "{marker}"
        "#},
        Upload::RateLimited => formatdoc! {r#"
            curl -fsS "{uri}/__upload/{name}" > /dev/null
            echo "{RATE_LIMITED}" >&2
            exit 101
        "#},
    };
    let query_body = match upload {
        Upload::Listed => "echo 0.4.0\n".to_owned(),
        Upload::Succeeds | Upload::RateLimited => formatdoc! {r#"
            test -f "{marker}" && echo 0.4.0
        "#},
    };
    fs::create_dir_all(root.join(name)).unwrap();
    fs::write(
        root.join(name).join("Cargo.toml"),
        formatdoc! {r#"
            [package]
            name = "{name}"
            version = "0.4.0"
        "#},
    )
    .unwrap();
    fs::write(
        root.join(name).join("CHANGELOG.md"),
        formatdoc! {"
            # Changelog

            ## {name} 0.4.0

            - First.
        "},
    )
    .unwrap();
    formatdoc! {"
        - name: {name}
          path: {name}/Cargo.toml
          publish: {publish}
          query: {query}
        ",
        publish = script(root, &format!("upload-{name}.sh"), &upload_body),
        query = script(root, &format!("query-{name}.sh"), &query_body),
    }
}

/// Crates at `0.4.0` in config order, each as [`crate_entry`] writes it,
/// sharing tag [`TAG`]. With `assets`, that tag carries the `linux-x64`
/// tarball [`TARBALL`]. Returns HEAD, the tip of `origin/main`.
fn crates(root: &Path, uri: &str, packages: &[(&str, Upload)], assets: bool) -> String {
    let entries: String = packages
        .iter()
        .map(|(name, upload)| crate_entry(root, uri, name, *upload))
        .collect();
    let targets = if assets {
        indoc! {"
            assets:
              linux-x64: {}
        "}
    } else {
        ""
    };
    common::write_config(
        root,
        &formatdoc! {"
            packages:
            {entries}{targets}"},
    );
    common::on_default_branch(root);
    common::git(root, &["rev-parse", "HEAD"])
}

fn not_found() -> ResponseTemplate {
    ResponseTemplate::new(404).set_body_json(serde_json::json!({
        "message": "Not Found",
        "documentation_url": "https://docs.github.com"
    }))
}

/// A Release as GitHub returns it, complete enough for octocrab to read:
/// release `id` for `tag` at `target`, holding `assets`.
/// octocrab reads a full asset, so a Release it reads holds none.
fn full_release(
    uri: &str,
    id: u64,
    tag: &str,
    target: &str,
    draft: bool,
    assets: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "url": format!("{uri}/repos/acme/demo/releases/{id}"),
        "html_url": format!("https://github.com/acme/demo/releases/tag/{tag}"),
        "assets_url": format!("{uri}/repos/acme/demo/releases/{id}/assets"),
        "upload_url": format!("{uri}/upload/repos/acme/demo/releases/{id}/assets{{?name,label}}"),
        "tarball_url": null,
        "zipball_url": null,
        "id": id,
        "node_id": "R",
        "tag_name": tag,
        "target_commitish": target,
        "name": tag,
        "body": "notes",
        "draft": draft,
        "prerelease": false,
        "created_at": "2026-11-13T00:00:00Z",
        "published_at": null,
        "assets": assets
            .iter()
            .map(|name| serde_json::json!({ "name": name, "state": "uploaded" }))
            .collect::<Vec<_>>()
    })
}

/// An asset as GitHub returns it after an upload.
fn uploaded_asset(uri: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "url": format!("{uri}/repos/acme/demo/releases/assets/9"),
        "browser_download_url": format!("https://github.com/acme/demo/releases/download/{TAG}/{name}"),
        "id": 9,
        "node_id": "A",
        "name": name,
        "label": null,
        "state": "uploaded",
        "content_type": "application/gzip",
        "size": 7,
        "digest": null,
        "download_count": 0,
        "created_at": "2026-11-13T00:00:00Z",
        "updated_at": "2026-11-13T00:00:00Z",
        "uploader": null
    })
}

/// The tag ref GitHub answers with once tag `tag` names `sha`.
fn tag_ref(tag: &str, sha: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "ref": format!("refs/tags/{tag}"),
        "node_id": "R",
        "url": format!("https://api.github.com/repos/acme/demo/git/refs/tags/{tag}"),
        "object": {
            "type": "commit",
            "sha": sha,
            "url": format!("https://api.github.com/repos/acme/demo/git/commits/{sha}")
        }
    }))
}

/// Tag `tag` missing for the first `missing` reads, then naming `sha`, as
/// GitHub serves a tag once publishing the draft created it.
async fn tag_after(server: &MockServer, tag: &str, sha: &str, missing: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{tag}")))
        .respond_with(not_found())
        .up_to_n_times(missing)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{tag}")))
        .respond_with(tag_ref(tag, sha))
        .mount(server)
        .await;
}

/// `GET /releases` answering `releases`, the only listing that shows drafts.
async fn listed(server: &MockServer, releases: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(releases))
        .mount(server)
        .await;
}

/// The draft `id` as `GET /releases/{id}` returns it, and the update that
/// publishes it.
async fn draft_to_publish(server: &MockServer, id: u64, head: &str, assets: &[&str]) {
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/{id}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(full_release(&uri, id, TAG, head, true, assets)),
        )
        .mount(server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(format!("/repos/acme/demo/releases/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
            &uri,
            id,
            TAG,
            head,
            false,
            &[],
        )))
        .mount(server)
        .await;
}

/// Mount on `server` a GitHub with no tag and no Release for [`TAG`]. It
/// takes a new draft (id 1), an upload of each of `assets` to it, and the
/// update that publishes it, and serves the tag at `head` once the plan and
/// the run have read it missing. Every `/__upload/{name}` answers 200.
/// Each upload reads the draft by id twice, once for verctl and once for
/// octocrab, before the read that publishes it sees the assets.
async fn new_release(server: &MockServer, head: &str, assets: &[&str]) {
    let uri = server.uri();
    tag_after(server, TAG, head, 2).await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(not_found())
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(201).set_body_json(full_release(
            &uri,
            1,
            TAG,
            head,
            true,
            &[],
        )))
        .mount(server)
        .await;
    if !assets.is_empty() {
        Mock::given(method("GET"))
            .and(path("/repos/acme/demo/releases/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
                &uri,
                1,
                TAG,
                head,
                true,
                &[],
            )))
            .up_to_n_times(2 * u64::try_from(assets.len()).unwrap())
            .mount(server)
            .await;
        for name in assets {
            Mock::given(method("POST"))
                .and(path("/upload/repos/acme/demo/releases/1/assets"))
                .and(query_param("name", *name))
                .respond_with(ResponseTemplate::new(201).set_body_json(uploaded_asset(&uri, name)))
                .mount(server)
                .await;
        }
    }
    draft_to_publish(server, 1, head, assets).await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/__upload/[a-z]+$"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

/// The directory `--assets-dir` names, holding each of `names`.
fn tarballs(names: &[&str]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for name in names {
        fs::write(dir.path().join(name), "tarball").unwrap();
    }
    dir
}

/// Each request the mock received, as its method and path.
async fn requests(server: &MockServer) -> Vec<(String, String)> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| (request.method.to_string(), request.url.path().to_owned()))
        .collect()
}

/// Where the first request with `method` to `path` sits among `seen`.
fn first(seen: &[(String, String)], method: &str, path: &str) -> usize {
    seen.iter()
        .position(|(by, to)| by == method && to == path)
        .unwrap_or_else(|| panic!("no {method} {path} in {seen:?}"))
}

/// Whether `seen` holds a request with `method` whose path starts with
/// `prefix`.
fn sent(seen: &[(String, String)], method: &str, prefix: &str) -> bool {
    seen.iter()
        .any(|(by, to)| by == method && to.starts_with(prefix))
}

/// `release` with `--color never` and `args`, against `server`.
fn release(root: &Path, server: &MockServer, args: &[&str]) -> Output {
    let mut all = vec!["release", "--color", "never"];
    all.extend(args);
    run(root, server, &all)
}

/// The run drafts the Release and uploads its asset, publishes `core` then
/// `app`, then publishes the draft at HEAD, and only then reads the tag. No
/// tag is created through the git refs API.
#[tokio::test]
async fn a_release_drafts_uploads_publishes_then_tags() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Succeeds), ("app", Upload::Succeeds)],
        true,
    );
    new_release(&server, &head, &[TARBALL]).await;
    let dir = tarballs(&[TARBALL]);
    let output = release(
        root.path(),
        &server,
        &["--assets-dir", dir.path().to_str().unwrap()],
    );
    assert!(output.status.success(), "{output:?}");
    let seen = requests(&server).await;
    let draft = first(&seen, "POST", "/repos/acme/demo/releases");
    let asset = first(&seen, "POST", "/upload/repos/acme/demo/releases/1/assets");
    let core = first(&seen, "GET", "/__upload/core");
    let app = first(&seen, "GET", "/__upload/app");
    let publish = first(&seen, "PATCH", "/repos/acme/demo/releases/1");
    let tag = format!("/repos/acme/demo/git/ref/tags/{TAG}");
    let proof = seen
        .iter()
        .rposition(|(by, to)| by == "GET" && *to == tag)
        .unwrap();
    assert!(
        draft < asset && asset < core && core < app && app < publish && publish < proof,
        "{seen:?}"
    );
    assert!(
        !sent(&seen, "POST", "/repos/acme/demo/git/refs"),
        "{seen:?}"
    );
    let received = server.received_requests().await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&received[draft].body).unwrap();
    assert_eq!(created["draft"], true, "{created}");
    assert_eq!(created["target_commitish"], head.as_str(), "{created}");
    let published: serde_json::Value = serde_json::from_slice(&received[publish].body).unwrap();
    assert_eq!(published["draft"], false, "{published}");
    assert_eq!(published["target_commitish"], head.as_str(), "{published}");
}

/// crates.io's 429 on the second crate stops the run with the draft holding
/// its asset: the draft is never published and no tag is created. The only
/// update writes the stop record into the draft's notes.
#[tokio::test]
async fn a_rate_limit_on_the_second_crate_leaves_the_draft_and_no_tag() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Succeeds), ("app", Upload::RateLimited)],
        true,
    );
    new_release(&server, &head, &[TARBALL]).await;
    let dir = tarballs(&[TARBALL]);
    let output = release(
        root.path(),
        &server,
        &["--assets-dir", dir.path().to_str().unwrap()],
    );
    assert!(!output.status.success(), "{output:?}");
    let said = String::from_utf8_lossy(&output.stderr)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        said.contains("try again after Fri, 13 Nov 2026 19:00:00 GMT"),
        "{said}"
    );
    assert!(said.contains("No tag exists"), "{said}");
    let seen = requests(&server).await;
    assert!(
        sent(&seen, "POST", "/upload/repos/acme/demo/releases/1/assets"),
        "{seen:?}"
    );
    assert!(sent(&seen, "GET", "/__upload/app"), "{seen:?}");
    assert!(
        !sent(&seen, "POST", "/repos/acme/demo/git/refs"),
        "{seen:?}"
    );
    let updates: Vec<serde_json::Value> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.method.as_str() == "PATCH")
        .map(|request| serde_json::from_slice(&request.body).unwrap())
        .collect();
    let [update] = updates.as_slice() else {
        panic!("one PATCH, the stop record: {updates:?}");
    };
    assert!(update.get("draft").is_none(), "{update}");
    let body = update["body"].as_str().unwrap();
    let record = body
        .lines()
        .find_map(|line| line.strip_prefix("<!-- verctl:stop "))
        .and_then(|rest| rest.strip_suffix(" -->"))
        .unwrap_or_else(|| panic!("no stop record in {body}"));
    let record: serde_json::Value = serde_json::from_str(record).unwrap();
    assert_eq!(
        record,
        serde_json::json!({
            "package": "app",
            "version": "0.4.0",
            "cause": "429 Too Many Requests",
            "retry_after": "Fri, 13 Nov 2026 19:00:00 GMT"
        })
    );
}

/// A rerun skips the crate the registry already lists, publishes the other
/// two in config order, then publishes the draft.
#[tokio::test]
async fn a_rerun_skips_every_crate_the_registry_lists() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[
            ("core", Upload::Succeeds),
            ("util", Upload::Listed),
            ("app", Upload::Succeeds),
        ],
        false,
    );
    new_release(&server, &head, &[]).await;
    let output = release(root.path(), &server, &[]);
    assert!(output.status.success(), "{output:?}");
    let seen = requests(&server).await;
    assert!(!sent(&seen, "GET", "/__upload/util"), "{seen:?}");
    let core = first(&seen, "GET", "/__upload/core");
    let app = first(&seen, "GET", "/__upload/app");
    let publish = first(&seen, "PATCH", "/repos/acme/demo/releases/1");
    assert!(core < app && app < publish, "{seen:?}");
}

/// A published Release at HEAD, its tag at HEAD, and every crate listed is
/// nothing to release: the run writes nothing.
#[tokio::test]
async fn a_finished_release_writes_nothing() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Listed), ("app", Upload::Listed)],
        false,
    );
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{TAG}")))
        .respond_with(tag_ref(TAG, &head))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
            &server.uri(),
            1,
            TAG,
            &head,
            false,
            &[],
        )))
        .mount(&server)
        .await;
    let output = release(root.path(), &server, &[]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nothing to release"), "{stdout}");
    let seen = requests(&server).await;
    assert!(
        !seen
            .iter()
            .any(|(by, _)| by == "POST" || by == "PATCH" || by == "DELETE"),
        "{seen:?}"
    );
}

/// A published Release whose tag names an older commit at the same version,
/// with the registries incomplete, fails before any upload, naming the tag and
/// both commits.
#[tokio::test]
async fn a_tag_on_another_commit_fails_before_any_upload() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let older = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Succeeds), ("app", Upload::Succeeds)],
        false,
    );
    fs::write(root.path().join("README"), "later\n").unwrap();
    let head = common::commit_all(root.path(), "docs: later");
    common::git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", &head],
    );
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{TAG}")))
        .respond_with(tag_ref(TAG, &older))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
            &server.uri(),
            1,
            TAG,
            &older,
            false,
            &[],
        )))
        .mount(&server)
        .await;
    let output = release(root.path(), &server, &[]);
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("tag {TAG}")), "{stderr}");
    assert!(stderr.contains(&older), "{stderr}");
    assert!(stderr.contains(&head), "{stderr}");
    let seen = requests(&server).await;
    assert!(!sent(&seen, "GET", "/__upload/"), "{seen:?}");
    assert!(
        !seen
            .iter()
            .any(|(by, _)| by == "POST" || by == "PATCH" || by == "DELETE"),
        "{seen:?}"
    );
}

/// A draft at another commit holds tarballs built from other code, so it is
/// deleted and a draft is created at HEAD, which the run publishes.
#[tokio::test]
async fn a_stale_draft_is_deleted_and_created_again_at_head() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Listed)],
        false,
    );
    let uri = server.uri();
    let stale = "1111111111111111111111111111111111111111";
    tag_after(&server, TAG, &head, 2).await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(not_found())
        .mount(&server)
        .await;
    listed(
        &server,
        serde_json::json!([full_release(&uri, 1, TAG, stale, true, &[])]),
    )
    .await;
    Mock::given(method("DELETE"))
        .and(path("/repos/acme/demo/releases/1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(201).set_body_json(full_release(
            &uri,
            2,
            TAG,
            &head,
            true,
            &[],
        )))
        .expect(1)
        .mount(&server)
        .await;
    draft_to_publish(&server, 2, &head, &[]).await;
    let output = release(root.path(), &server, &[]);
    assert!(output.status.success(), "{output:?}");
    let seen = requests(&server).await;
    let deleted = first(&seen, "DELETE", "/repos/acme/demo/releases/1");
    let created = first(&seen, "POST", "/repos/acme/demo/releases");
    let published = first(&seen, "PATCH", "/repos/acme/demo/releases/2");
    assert!(deleted < created && created < published, "{seen:?}");
    let received = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received[created].body).unwrap();
    assert_eq!(body["draft"], true, "{body}");
    assert_eq!(body["target_commitish"], head.as_str(), "{body}");
}

/// A declared tarball that is neither on the draft nor in `--assets-dir`
/// fails the run before it writes anything.
#[tokio::test]
async fn a_missing_tarball_fails_before_any_write() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Succeeds), ("app", Upload::Succeeds)],
        true,
    );
    new_release(&server, &head, &[TARBALL]).await;
    let dir = tarballs(&[]);
    let output = release(
        root.path(),
        &server,
        &["--assets-dir", dir.path().to_str().unwrap()],
    );
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--assets-dir"), "{stderr}");
    assert!(stderr.contains(TARBALL), "{stderr}");
    let seen = requests(&server).await;
    assert!(
        !seen
            .iter()
            .any(|(by, _)| by == "POST" || by == "PATCH" || by == "DELETE"),
        "{seen:?}"
    );
    assert!(!sent(&seen, "GET", "/__upload/"), "{seen:?}");
}

/// On a rerun the draft at HEAD already holds the declared tarball, so an
/// empty `--assets-dir` is enough: nothing is uploaded, and the draft is
/// published.
#[tokio::test]
async fn a_draft_at_head_that_holds_every_tarball_needs_none_in_the_directory() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Listed)],
        true,
    );
    let uri = server.uri();
    tag_after(&server, TAG, &head, 2).await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(not_found())
        .mount(&server)
        .await;
    listed(
        &server,
        serde_json::json!([full_release(&uri, 1, TAG, &head, true, &[TARBALL])]),
    )
    .await;
    draft_to_publish(&server, 1, &head, &[TARBALL]).await;
    let dir = tarballs(&[]);
    let output = release(
        root.path(),
        &server,
        &["--assets-dir", dir.path().to_str().unwrap()],
    );
    assert!(output.status.success(), "{output:?}");
    let seen = requests(&server).await;
    assert!(!sent(&seen, "POST", "/"), "{seen:?}");
    first(&seen, "PATCH", "/repos/acme/demo/releases/1");
}

/// A mutable Release published at HEAD that lacks a declared tarball takes it
/// from `--assets-dir`: the run uploads it, opens no draft, and publishes
/// nothing.
#[tokio::test]
async fn a_mutable_published_release_takes_the_tarball_it_lacks() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Listed)],
        true,
    );
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{TAG}")))
        .respond_with(tag_ref(TAG, &head))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
            &uri,
            1,
            TAG,
            &head,
            false,
            &[],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
            &uri,
            1,
            TAG,
            &head,
            false,
            &[],
        )))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/upload/repos/acme/demo/releases/1/assets"))
        .and(query_param("name", TARBALL))
        .respond_with(ResponseTemplate::new(201).set_body_json(uploaded_asset(&uri, TARBALL)))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tarballs(&[TARBALL]);
    let output = release(
        root.path(),
        &server,
        &["--assets-dir", dir.path().to_str().unwrap()],
    );
    assert!(output.status.success(), "{output:?}");
    let seen = requests(&server).await;
    first(&seen, "POST", "/upload/repos/acme/demo/releases/1/assets");
    assert!(
        !sent(&seen, "POST", "/repos/acme/demo/releases"),
        "{seen:?}"
    );
    assert!(!sent(&seen, "PATCH", "/"), "{seen:?}");
    assert!(!sent(&seen, "DELETE", "/"), "{seen:?}");
}

/// A stock cargo crate `demo` at `0.4.0`, whose upload check reads the
/// crates.io index, with its changelog section, and `.ctl/ver.yaml` holding
/// `top` before its packages. Committed as the tip of `main`; returns HEAD.
fn proof_crate(root: &Path, top: &str) -> String {
    common::write_config(
        root,
        &formatdoc! {"
            {top}packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(root.join("Cargo.toml"), manifest("0.4.0")).unwrap();
    fs::write(
        root.join("CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## demo 0.4.0

            - First.
        "},
    )
    .unwrap();
    common::on_default_branch(root);
    common::git(root, &["rev-parse", "HEAD"])
}

/// A crates.io whose index lists `demo` 0.4.0, so no upload runs, and whose
/// API lists `owners` as the crate's user owners and a token upload as its
/// newest version. Returns the index and the API, two servers, so a test can
/// tell which one a request reached.
async fn crates_io(owners: &[&str]) -> (MockServer, MockServer) {
    let index = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/de/mo/demo"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                serde_json::json!({ "name": "demo", "vers": "0.4.0" }).to_string(),
            ),
        )
        .mount(&index)
        .await;
    let api = MockServer::start().await;
    let users: Vec<serde_json::Value> = owners
        .iter()
        .enumerate()
        .map(|(id, login)| {
            serde_json::json!({
                "id": id,
                "login": login,
                "kind": "user",
                "url": format!("https://github.com/{login}"),
                "name": null,
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/crates/demo/owners"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "users": users })),
        )
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/crates/demo/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "versions": [{ "num": "0.4.0", "trustpub_data": null }],
            "meta": { "total": 1, "next_page": null },
        })))
        .mount(&api)
        .await;
    (index, api)
}

/// `release` with `args` against `github`, reading crates.io from `index`
/// and `api` through `--crates-io-index` and `--crates-io-api`. The report is
/// laid out wide, so no message wraps inside a word.
fn release_against(
    root: &Path,
    github: &MockServer,
    (index, api): &(MockServer, MockServer),
    args: &[&str],
) -> Output {
    off_actions()
        .current_dir(root)
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_API_URL", github.uri())
        .env("COLUMNS", "400")
        .args(["release", "--color", "never"])
        .arg("--crates-io-index")
        .arg(index.uri())
        .arg("--crates-io-api")
        .arg(api.uri())
        .args(args)
        .output()
        .unwrap()
}

/// stdout and stderr of `output`, with every run of whitespace one space, so
/// a message the terminal wraps still reads as one line.
fn printed(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

/// `demo` lost owner `any-victor` after the merge: the `registry` check of
/// the release proof fails, and the run stops naming the crate and the login
/// before the draft. GitHub takes no write and no upload runs.
#[tokio::test]
async fn a_missing_owner_stops_the_release_before_the_draft() {
    let root = TempDir::new().unwrap();
    let head = proof_crate(root.path(), "owners: [any-victor]\n");
    let github = MockServer::start().await;
    new_release(&github, &head, &[]).await;
    let registry = crates_io(&["alice"]).await;
    let output = release_against(root.path(), &github, &registry, &[]);
    assert!(!output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains("the release proof failed, and nothing was published"),
        "{said}"
    );
    assert!(
        said.contains("demo: lacks owner any-victor on crates.io, which lists alice"),
        "{said}"
    );
    let seen = requests(&github).await;
    for write in ["POST", "PATCH", "DELETE"] {
        assert!(!sent(&seen, write, "/"), "{write}: {seen:?}");
    }
    assert!(!sent(&seen, "GET", "/__upload/"), "{seen:?}");
}

/// With the default `release_checks` and no `VERCTL_OWNER_TOKEN`, the run
/// proves `registry`, reports `trust` as skipped naming the variable, and
/// goes on to release. No request reads the trusted publishers.
#[tokio::test]
async fn trust_is_skipped_without_the_owner_token() {
    let root = TempDir::new().unwrap();
    let head = proof_crate(root.path(), "owners: [any-victor]\n");
    let github = MockServer::start().await;
    new_release(&github, &head, &[]).await;
    let registry = crates_io(&["any-victor"]).await;
    let output = release_against(root.path(), &github, &registry, &[]);
    assert!(output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains("skipped trust, VERCTL_OWNER_TOKEN is unset"),
        "{said}"
    );
    let asked = requests(&registry.1).await;
    assert!(sent(&asked, "GET", "/crates/demo/owners"), "{asked:?}");
    assert!(
        !sent(&asked, "GET", "/trusted_publishing/github_configs"),
        "{asked:?}"
    );
    let seen = requests(&github).await;
    assert!(
        sent(&seen, "PATCH", "/repos/acme/demo/releases/1"),
        "{seen:?}"
    );
}

/// `release_checks: []` turns the proof off: the run releases, and the
/// crates.io API receives no request.
#[tokio::test]
async fn an_empty_release_checks_runs_no_proof() {
    let root = TempDir::new().unwrap();
    let head = proof_crate(
        root.path(),
        indoc! {"
            owners: [any-victor]
            release_checks: []
        "},
    );
    let github = MockServer::start().await;
    new_release(&github, &head, &[]).await;
    let registry = crates_io(&["alice"]).await;
    let output = release_against(root.path(), &github, &registry, &[]);
    assert!(output.status.success(), "{output:?}");
    let asked = requests(&registry.1).await;
    assert!(asked.is_empty(), "{asked:?}");
    let seen = requests(&github).await;
    assert!(sent(&seen, "POST", "/repos/acme/demo/releases"), "{seen:?}");
}

/// Fail unless every request in `seen` read, so a dry run made no write: no
/// draft, no upload, no tag, no update.
fn assert_only_reads(seen: &[(String, String)]) {
    assert!(seen.iter().all(|(by, _)| by == "GET"), "{seen:?}");
    assert!(!sent(seen, "GET", "/__upload/"), "{seen:?}");
}

/// `release --dry-run` with a missing owner prints the plan, fails the same
/// `registry` check naming the crate, and writes nothing.
#[tokio::test]
async fn a_dry_run_runs_the_proof() {
    let root = TempDir::new().unwrap();
    let head = proof_crate(root.path(), "owners: [any-victor]\n");
    let github = MockServer::start().await;
    new_release(&github, &head, &[]).await;
    let registry = crates_io(&["alice"]).await;
    let output = release_against(root.path(), &github, &registry, &["--dry-run"]);
    assert!(!output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(said.contains("release would create v0.4.0"), "{said}");
    assert!(
        said.contains("demo: lacks owner any-victor on crates.io, which lists alice"),
        "{said}"
    );
    assert_only_reads(&requests(&github).await);
    assert_only_reads(&requests(&registry.0).await);
    assert_only_reads(&requests(&registry.1).await);
}

/// A tag at another commit fails the dry run as it fails the run, with the
/// same message, before any upload or write.
#[tokio::test]
async fn a_dry_run_fails_on_a_tag_at_another_commit() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let older = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Succeeds), ("app", Upload::Succeeds)],
        false,
    );
    fs::write(root.path().join("README"), "later\n").unwrap();
    let head = common::commit_all(root.path(), "docs: later");
    common::git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", &head],
    );
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{TAG}")))
        .respond_with(tag_ref(TAG, &older))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
            &server.uri(),
            1,
            TAG,
            &older,
            false,
            &[],
        )))
        .mount(&server)
        .await;
    let output = release(root.path(), &server, &["--dry-run"]);
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("tag {TAG} names {older}, not HEAD {head}")),
        "{stderr}"
    );
    assert_only_reads(&requests(&server).await);
}

/// A workflow change at the default branch fails the dry run with the
/// workflow message, before any token or read, and `--allow-workflow-change`
/// lets it plan the release and list the proof it went past.
#[tokio::test]
async fn a_dry_run_fails_on_a_workflow_change_unless_allowed() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Succeeds)],
        false,
    );
    let workflow = root.path().join(".github/workflows/ci.yml");
    fs::create_dir_all(workflow.parent().unwrap()).unwrap();
    fs::write(&workflow, "on: push\n").unwrap();
    let at_release = common::commit_all(root.path(), "ci: push");
    fs::write(&workflow, "on: [push, pull_request]\n").unwrap();
    let tip = common::commit_all(root.path(), "ci: run on pull requests");
    common::git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", &tip],
    );
    common::git(root.path(), &["checkout", "-q", "--detach", &at_release]);
    new_release(&server, &at_release, &[]).await;
    let refused = release(root.path(), &server, &["--dry-run"]);
    assert!(!refused.status.success(), "{refused:?}");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains(".github/workflows/"), "{stderr}");
    assert!(stderr.contains("--allow-workflow-change"), "{stderr}");
    assert!(requests(&server).await.is_empty());
    let allowed = release(
        root.path(),
        &server,
        &["--dry-run", "--allow-workflow-change"],
    );
    assert!(allowed.status.success(), "{allowed:?}");
    let said = printed(&allowed);
    assert!(said.contains("allowed workflow-change"), "{said}");
    assert!(said.contains("release would create v0.4.0"), "{said}");
    assert_only_reads(&requests(&server).await);
}

/// The dry run's report as JSON, in `root` against `server`.
fn dry_run_json(root: &Path, server: &MockServer) -> serde_json::Value {
    let output = run(
        root,
        server,
        &[
            "--format",
            "json",
            "--color",
            "never",
            "release",
            "--dry-run",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// A version the registry lists is reported as present and plans no upload,
/// and the crate it still lacks is planned.
#[tokio::test]
async fn a_dry_run_reports_a_listed_version_as_present() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Listed), ("app", Upload::Succeeds)],
        false,
    );
    new_release(&server, &head, &[]).await;
    let report = dry_run_json(root.path(), &server);
    assert_eq!(report["outcome"], "released");
    let packages = report["packages"].as_array().unwrap();
    let rows: Vec<(&str, &serde_json::Value)> = packages
        .iter()
        .map(|row| (row["name"].as_str().unwrap(), &row["note"]))
        .collect();
    assert_eq!(
        rows,
        [
            ("core", &serde_json::json!("already")),
            ("app", &serde_json::Value::Null)
        ]
    );
    assert_eq!(
        report["releases"],
        serde_json::json!(["would create v0.4.0"])
    );
    assert_only_reads(&requests(&server).await);
}

/// A Release published at HEAD that lacks a declared tarball is reported as
/// completed, not created, and the run needs no `--assets-dir` to say so.
#[tokio::test]
async fn a_dry_run_reports_a_published_release_as_completed() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let head = crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Listed)],
        true,
    );
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{TAG}")))
        .respond_with(tag_ref(TAG, &head))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{TAG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(full_release(
            &server.uri(),
            1,
            TAG,
            &head,
            false,
            &[],
        )))
        .mount(&server)
        .await;
    let report = dry_run_json(root.path(), &server);
    assert_eq!(
        report["releases"],
        serde_json::json!(["would complete v0.4.0"])
    );
    assert_only_reads(&requests(&server).await);
}

/// The dry run reads GitHub as the run does, so it fails without a token and
/// names the variable, before any request.
#[tokio::test]
async fn a_dry_run_without_a_token_fails_naming_it() {
    let root = TempDir::new().unwrap();
    let server = MockServer::start().await;
    crates(
        root.path(),
        &server.uri(),
        &[("core", Upload::Succeeds)],
        false,
    );
    let output = off_actions()
        .current_dir(root.path())
        .env("GITHUB_API_URL", server.uri())
        .args(["release", "--color", "never", "--dry-run"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
    assert!(requests(&server).await.is_empty());
}
