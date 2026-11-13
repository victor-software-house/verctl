//! A Release with declared assets is a draft until every asset is uploaded,
//! as `status` and `release` see it through the binary.

mod common;

use std::fs;
use std::path::Path;
use std::process::Output;

use indoc::indoc;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
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

fn run(root: &Path, server: &MockServer, args: &[&str]) -> Output {
    common::verctl()
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
            r#"assets={{"include":[{{"commit":"{second}","id":"linux-x64","labels":["ubuntu-26.04"],"tag":"v1.1.0"}}]}}"#
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
