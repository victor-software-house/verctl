//! `release bootstrap` and `release trust` through the binary, against a
//! recorded crates.io, a stand-in `cargo`, and a recorded GitHub.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fixture setup unwraps; a broken fixture fails the test"
)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::{env, fs};

use indoc::formatdoc;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// The owner's token, which the entry requests carry as `Authorization`.
const OWNER_TOKEN: &str = "t0ken";

/// Both tokens a real `release bootstrap` needs.
const BOTH: [(&str, &str); 2] = [
    ("VERCTL_REGISTRY_TOKEN", "r3gistry"),
    ("VERCTL_OWNER_TOKEN", OWNER_TOKEN),
];

/// Everything a test talks to: the repository, a `cargo` that records its
/// calls, and one server each for the crates.io API, its index, and GitHub.
struct World {
    root: TempDir,
    tools: TempDir,
    api: MockServer,
    index: MockServer,
    github: MockServer,
}

impl World {
    async fn new() -> Self {
        Self {
            root: TempDir::new().unwrap(),
            tools: TempDir::new().unwrap(),
            api: MockServer::start().await,
            index: MockServer::start().await,
            github: MockServer::start().await,
        }
    }

    fn repo(&self) -> &Path {
        self.root.path()
    }

    /// The file whose presence marks `name` as published.
    fn marker(&self, name: &str) -> PathBuf {
        self.tools.path().join(format!("{name}.up"))
    }

    /// The arguments of each `cargo` call that was not `cargo metadata`.
    fn cargo_calls(&self) -> Vec<String> {
        fs::read_to_string(self.tools.path().join("cargo.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Each upload with the registry token its publisher got, as
    /// `<crate> <token>`, sorted.
    fn token_calls(&self) -> Vec<String> {
        let mut calls: Vec<String> = fs::read_to_string(self.tools.path().join("tokens.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        calls.sort();
        calls
    }

    /// A `cargo` first on `PATH`: `metadata` goes to the real one, and every
    /// other call is logged with the registry token its publisher got, and
    /// marks the crate it names as published.
    fn fake_cargo(&self) {
        let real = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let tools = self.tools.path().display();
        let script = formatdoc! {r#"
            #!/bin/sh
            if [ "$1" = metadata ]; then exec "{real}" "$@"; fi
            echo "$*" >> "{tools}/cargo.log"
            crate=
            prev=
            for arg in "$@"; do
                case "$prev" in
                    --manifest-path) crate=$(basename "$(dirname "$arg")") ;;
                    --package) crate=$arg ;;
                esac
                prev=$arg
            done
            echo "$crate $CARGO_REGISTRY_TOKEN" >> "{tools}/tokens.log"
            touch "{tools}/$crate.up"
        "#};
        let bin = self.tools.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let cargo = bin.join("cargo");
        fs::write(&cargo, script).unwrap();
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Standalone crates, each a package at `<name>/Cargo.toml`, committed as the
/// tip of `main`.
fn standalone(root: &Path, crates: &[(&str, &str)]) {
    let packages: String = crates
        .iter()
        .map(|(name, _)| {
            formatdoc! {"
                - name: {name}
                  path: {name}/Cargo.toml
            "}
        })
        .collect();
    common::write_config(root, &format!("packages:\n{packages}"));
    for (name, version) in crates {
        fs::create_dir_all(root.join(name).join("src")).unwrap();
        fs::write(root.join(name).join("src/lib.rs"), "").unwrap();
        fs::write(
            root.join(name).join("Cargo.toml"),
            formatdoc! {r#"
                [package]
                name = "{name}"
                version = "{version}"
                edition = "2024"
            "#},
        )
        .unwrap();
    }
    common::on_default_branch(root);
}

/// A virtual workspace, one package `demo` at `Cargo.toml`, whose members
/// share the version `0.7.0`, committed as the tip of `main`.
fn workspace(root: &Path, members: &[&str]) {
    common::write_config(root, "packages:\n  - name: demo\n    path: Cargo.toml\n");
    fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## demo 0.7.0\n\n- First.\n",
    )
    .unwrap();
    let listed = members
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join("Cargo.toml"),
        formatdoc! {r#"
            [workspace]
            resolver = "3"
            members = [{listed}]

            [workspace.package]
            version = "0.7.0"
            edition = "2024"
        "#},
    )
    .unwrap();
    for name in members {
        fs::create_dir_all(root.join(name).join("src")).unwrap();
        fs::write(root.join(name).join("src/lib.rs"), "").unwrap();
        fs::write(
            root.join(name).join("Cargo.toml"),
            formatdoc! {r#"
                [package]
                name = "{name}"
                version.workspace = true
                edition.workspace = true
            "#},
        )
        .unwrap();
    }
    common::on_default_branch(root);
}

/// Answers `body` once the marker file exists, and 404 before.
struct After {
    marker: PathBuf,
    body: ResponseTemplate,
}

impl Respond for After {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        if self.marker.exists() {
            self.body.clone()
        } else {
            ResponseTemplate::new(404)
        }
    }
}

/// `name` at `version` on crates.io, in the API and the index: from the start
/// when `live`, and otherwise once the stand-in `cargo` publishes it.
async fn on_crates_io(world: &World, name: &str, version: &str, live: bool) {
    if live {
        fs::write(world.marker(name), "").unwrap();
    }
    let owners = json!({
        "users": [{
            "id": 1,
            "login": "any-victor",
            "kind": "user",
            "url": "https://github.com/any-victor",
            "name": null,
        }]
    });
    Mock::given(method("GET"))
        .and(path(format!("/crates/{name}/owners")))
        .respond_with(After {
            marker: world.marker(name),
            body: ResponseTemplate::new(200).set_body_json(owners),
        })
        .mount(&world.api)
        .await;
    index_after_publish(world, name, version).await;
    newest_version(world, name, version).await;
}

/// `name` is on crates.io, with another version or none listed: its owners
/// answer from the start, and `version` appears in the index once the
/// stand-in `cargo` publishes it.
async fn known_on_crates_io(world: &World, name: &str, version: &str) {
    let owners = json!({
        "users": [{
            "id": 1,
            "login": "any-victor",
            "kind": "user",
            "url": "https://github.com/any-victor",
            "name": null,
        }]
    });
    Mock::given(method("GET"))
        .and(path(format!("/crates/{name}/owners")))
        .respond_with(ResponseTemplate::new(200).set_body_json(owners))
        .mount(&world.api)
        .await;
    index_after_publish(world, name, version).await;
    newest_version(world, name, version).await;
}

/// The index lists `version` of `name` once its marker exists.
async fn index_after_publish(world: &World, name: &str, version: &str) {
    let record = json!({ "name": name, "vers": version }).to_string();
    Mock::given(method("GET"))
        .and(path(format!("/{}/{}/{name}", &name[..2], &name[2..4])))
        .respond_with(After {
            marker: world.marker(name),
            body: ResponseTemplate::new(200).set_body_string(record),
        })
        .mount(&world.index)
        .await;
}

/// The newest version of `name`, which the registry proof reads as evidence,
/// came by a token.
async fn newest_version(world: &World, name: &str, version: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/crates/{name}/versions")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "versions": [{ "num": version, "trustpub_data": null }],
            "meta": { "total": 1, "next_page": null },
        })))
        .mount(&world.api)
        .await;
}

/// `name` is not on crates.io: its owners are a 404 for good.
async fn absent(world: &World, name: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/crates/{name}/owners")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&world.api)
        .await;
}

/// One trusted publisher entry as crates.io lists it.
fn entry(owner: &str, repository: &str, workflow: &str, id: usize) -> Value {
    json!({
        "id": id,
        "crate": "demo",
        "repository_owner": owner,
        "repository_owner_id": 9,
        "repository_name": repository,
        "workflow_filename": workflow,
        "environment": null,
        "created_at": "2026-11-13T00:00:00Z",
    })
}

/// The entry `release trust` adds for `name`: this repository, `release.yml`,
/// and no environment.
fn entry_body(name: &str) -> Value {
    json!({
        "github_config": {
            "crate": name,
            "repository_owner": "acme",
            "repository_name": "demo",
            "workflow_filename": "release.yml",
            "environment": null,
        }
    })
}

/// crates.io lists `entries` as the trusted publishers of `name`, to the
/// owner's token.
async fn entries(server: &MockServer, name: &str, entries: Vec<Value>) {
    let total = entries.len();
    Mock::given(method("GET"))
        .and(path("/trusted_publishing/github_configs"))
        .and(query_param("crate", name))
        .and(header("authorization", OWNER_TOKEN))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "github_configs": entries,
            "meta": { "total": total, "next_page": null },
        })))
        .mount(server)
        .await;
}

/// crates.io takes the entry for `name`, and only that exact body.
async fn accepts(server: &MockServer, name: &str) {
    Mock::given(method("POST"))
        .and(path("/trusted_publishing/github_configs"))
        .and(header("authorization", OWNER_TOKEN))
        .and(body_json(entry_body(name)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "github_config": entry("acme", "demo", "release.yml", 1),
        })))
        .mount(server)
        .await;
}

/// verctl with no host `CI` or `GITHUB_*` variable, so each run sees only
/// the GitHub the test sets.
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

/// `release <step> <extra...>` in the world's repository, with the stand-in
/// `cargo` first on `PATH`, the API at the world's servers, and `envs` set.
fn run(world: &World, step: &str, envs: &[(&str, &str)], extra: &[&str]) -> Output {
    let mut command = base(world, envs);
    command
        .args(["--format", "json", "--color", "never", "release", step])
        .args(extra)
        .args(["--crates-io-api", &world.api.uri()]);
    if step == "bootstrap" {
        command.args(["--crates-io-index", &world.index.uri()]);
    }
    command.output().unwrap()
}

/// `release <extra...>`, the run itself, in the world's repository, reading
/// both crates.io servers.
fn release(world: &World, envs: &[(&str, &str)], extra: &[&str]) -> Output {
    let mut command = base(world, envs);
    command
        .args(["--format", "json", "--color", "never", "release"])
        .args(extra)
        .args(["--crates-io-api", &world.api.uri()])
        .args(["--crates-io-index", &world.index.uri()]);
    command.output().unwrap()
}

/// verctl in the world's repository, with the stand-in `cargo` first on
/// `PATH`, GitHub at the world's server, and `envs` set.
fn base(world: &World, envs: &[(&str, &str)]) -> Command {
    let path = format!(
        "{}:{}",
        world.tools.path().join("bin").display(),
        env::var("PATH").unwrap_or_default()
    );
    let mut command = off_actions();
    command
        .current_dir(world.repo())
        .env("PATH", path)
        .env("GITHUB_API_URL", world.github.uri())
        .env("COLUMNS", "400")
        .envs(envs.iter().copied());
    command
}

fn trust(world: &World, envs: &[(&str, &str)], extra: &[&str]) -> Output {
    run(world, "trust", envs, extra)
}

fn bootstrap(world: &World, envs: &[(&str, &str)], extra: &[&str]) -> Output {
    run(world, "bootstrap", envs, extra)
}

/// stdout and stderr of `output`, with every run of whitespace one space.
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

/// The rows of a report that printed JSON on stdout.
fn rows(output: &Output) -> Vec<Value> {
    assert!(output.status.success(), "{}", printed(output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    report["crates"].as_array().unwrap().clone()
}

/// The method and path of each request `server` got.
async fn received(server: &MockServer) -> Vec<(String, String)> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| (request.method.to_string(), request.url.path().to_owned()))
        .collect()
}

/// No request at all, as [`received`] reports it.
fn no_requests() -> Vec<(String, String)> {
    Vec::new()
}

/// How many requests of `method` `server` got.
async fn count(server: &MockServer, method: &str) -> usize {
    received(server)
        .await
        .iter()
        .filter(|(sent, _)| sent == method)
        .count()
}

/// A crate with no entry gets one `POST` with the exact body.
#[tokio::test]
async fn trust_adds_a_missing_entry() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.1.0")]);
    on_crates_io(&world, "demo", "0.1.0", true).await;
    entries(&world.api, "demo", Vec::new()).await;
    accepts(&world.api, "demo").await;
    let output = trust(&world, &[("VERCTL_OWNER_TOKEN", OWNER_TOKEN)], &[]);
    assert_eq!(rows(&output), [json!({ "name": "demo", "state": "added" })]);
    assert_eq!(count(&world.api, "POST").await, 1);
}

/// An entry that names the repository and the workflow is left alone, and a
/// second run sends no `POST`.
#[tokio::test]
async fn trust_leaves_a_matching_entry() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.1.0")]);
    on_crates_io(&world, "demo", "0.1.0", true).await;
    entries(
        &world.api,
        "demo",
        vec![entry("acme", "demo", "release.yml", 1)],
    )
    .await;
    let output = trust(&world, &[("VERCTL_OWNER_TOKEN", OWNER_TOKEN)], &[]);
    assert_eq!(
        rows(&output),
        [json!({ "name": "demo", "state": "already-set" })]
    );
    assert_eq!(count(&world.api, "POST").await, 0);
}

/// An entry for another repository stays, and this repository's entry is
/// added beside it: no `DELETE` and no `PATCH` is sent.
#[tokio::test]
async fn trust_adds_beside_a_foreign_entry() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.1.0")]);
    on_crates_io(&world, "demo", "0.1.0", true).await;
    entries(
        &world.api,
        "demo",
        vec![entry("other", "tool", "release.yml", 1)],
    )
    .await;
    accepts(&world.api, "demo").await;
    let output = trust(&world, &[("VERCTL_OWNER_TOKEN", OWNER_TOKEN)], &[]);
    assert_eq!(rows(&output), [json!({ "name": "demo", "state": "added" })]);
    assert_eq!(count(&world.api, "POST").await, 1);
    assert_eq!(count(&world.api, "DELETE").await, 0);
    assert_eq!(count(&world.api, "PATCH").await, 0);
}

/// `--dry-run` lists the entry it would add and sends no `POST`.
#[tokio::test]
async fn trust_dry_run_sends_no_post() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.1.0")]);
    on_crates_io(&world, "demo", "0.1.0", true).await;
    entries(&world.api, "demo", Vec::new()).await;
    let output = trust(
        &world,
        &[("VERCTL_OWNER_TOKEN", OWNER_TOKEN)],
        &["--dry-run"],
    );
    assert_eq!(
        rows(&output),
        [json!({ "name": "demo", "state": "would-add" })]
    );
    assert_eq!(count(&world.api, "POST").await, 0);
}

/// A crate that holds 5 entries, none of them this repository's, fails
/// naming the crate and the limit, and sends no `POST` for it.
#[tokio::test]
async fn trust_fails_a_crate_at_the_limit() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.1.0")]);
    on_crates_io(&world, "demo", "0.1.0", true).await;
    let foreign = (1..=5)
        .map(|id| entry("other", &format!("tool-{id}"), "release.yml", id))
        .collect();
    entries(&world.api, "demo", foreign).await;
    accepts(&world.api, "demo").await;
    let output = trust(&world, &[("VERCTL_OWNER_TOKEN", OWNER_TOKEN)], &[]);
    assert!(!output.status.success(), "{}", printed(&output));
    let text = printed(&output);
    assert!(
        text.contains("demo: holds 5 trusted publisher entries"),
        "{text}"
    );
    assert!(text.contains("the crates.io limit of 5"), "{text}");
    assert_eq!(count(&world.api, "POST").await, 0);
}

/// Without `VERCTL_OWNER_TOKEN` it fails naming the variable and sends no
/// request, a dry run included.
#[tokio::test]
async fn trust_needs_the_owner_token() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.1.0")]);
    on_crates_io(&world, "demo", "0.1.0", true).await;
    for extra in [&[][..], &["--dry-run"][..]] {
        let output = trust(&world, &[], extra);
        assert!(!output.status.success(), "{}", printed(&output));
        assert!(
            printed(&output).contains("VERCTL_OWNER_TOKEN"),
            "{}",
            printed(&output)
        );
    }
    assert_eq!(received(&world.api).await, no_requests());
}

/// A crate crates.io lacks fails with the bootstrap that publishes it, and
/// the crates after it are still handled.
#[tokio::test]
async fn trust_fails_a_missing_crate() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.0.0")]);
    absent(&world, "demo").await;
    let output = trust(&world, &[("VERCTL_OWNER_TOKEN", OWNER_TOKEN)], &[]);
    assert!(!output.status.success(), "{}", printed(&output));
    let text = printed(&output);
    assert!(
        text.contains("demo: missing from crates.io; run `verctl release bootstrap demo`"),
        "{text}"
    );
    assert_eq!(count(&world.api, "POST").await, 0);
}

/// GitHub answers 404 as it does for a tag or a Release it lacks.
fn gh_not_found() -> ResponseTemplate {
    ResponseTemplate::new(404).set_body_json(json!({
        "message": "Not Found",
        "documentation_url": "https://docs.github.com",
    }))
}

/// A Release as GitHub returns it, complete enough for octocrab to read.
fn release_json(uri: &str, tag: &str, target: &str, draft: bool) -> Value {
    json!({
        "url": format!("{uri}/repos/acme/demo/releases/1"),
        "html_url": format!("https://github.com/acme/demo/releases/tag/{tag}"),
        "assets_url": format!("{uri}/repos/acme/demo/releases/1/assets"),
        "upload_url": format!("{uri}/upload/repos/acme/demo/releases/1/assets{{?name,label}}"),
        "tarball_url": null,
        "zipball_url": null,
        "id": 1,
        "node_id": "R",
        "tag_name": tag,
        "target_commitish": target,
        "name": tag,
        "body": "notes",
        "draft": draft,
        "prerelease": false,
        "created_at": "2026-11-13T00:00:00Z",
        "published_at": null,
        "assets": [],
    })
}

/// GitHub holds no tag and no Release for `tag`. It takes a new draft, id 1,
/// and the update that publishes it, and serves the tag at `head` once the
/// plan and the run have read it missing.
async fn new_release(world: &World, tag: &str, head: &str) {
    let uri = world.github.uri();
    let server = &world.github;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{tag}")))
        .respond_with(gh_not_found())
        .up_to_n_times(2)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{tag}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ref": format!("refs/tags/{tag}"),
            "node_id": "R",
            "url": format!("https://api.github.com/repos/acme/demo/git/refs/tags/{tag}"),
            "object": {
                "type": "commit",
                "sha": head,
                "url": format!("https://api.github.com/repos/acme/demo/git/commits/{head}"),
            },
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/releases/tags/{tag}")))
        .respond_with(gh_not_found())
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(201).set_body_json(release_json(&uri, tag, head, true)))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(release_json(&uri, tag, head, true)))
        .mount(server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/demo/releases/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(release_json(&uri, tag, head, false)),
        )
        .mount(server)
        .await;
}

/// The body of the first request with `method` to `path` that `server` got.
async fn body_of(server: &MockServer, method: &str, path: &str) -> Value {
    let requests = server.received_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|request| request.method.as_str() == method && request.url.path() == path)
        .unwrap_or_else(|| panic!("no {method} {path}"));
    serde_json::from_slice(&request.body).unwrap()
}

/// The report of a run that printed JSON on stdout.
fn report(output: &Output) -> Value {
    assert!(output.status.success(), "{}", printed(output));
    serde_json::from_slice(&output.stdout).unwrap()
}

/// With both tokens set, `--new` creates the missing member with the
/// registry token and uploads the listed crate's other member with the
/// trusted one, notes the creation, and passes the proof with a notice.
#[tokio::test]
async fn new_run_creates_a_missing_member_with_the_registry_token() {
    let world = World::new().await;
    world.fake_cargo();
    workspace(world.repo(), &["demo-core", "demo-cli"]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    known_on_crates_io(&world, "demo-core", "0.7.0").await;
    on_crates_io(&world, "demo-cli", "0.7.0", false).await;
    new_release(&world, "v0.7.0", &head).await;
    let output = release(
        &world,
        &[
            ("VERCTL_REGISTRY_TOKEN", "r3gistry"),
            ("VERCTL_TRUSTED_PUBLISHING_TOKEN", "trust3d"),
            ("GITHUB_TOKEN", "gh-token"),
        ],
        &["--new"],
    );
    let report = report(&output);
    assert_eq!(
        world.token_calls(),
        ["demo-cli r3gistry", "demo-core trust3d"]
    );
    let created = report["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "demo-cli")
        .unwrap();
    assert_eq!(created["note"], "created with the registry token");
    let notice = json!({
        "row": "notice",
        "check": "registry",
        "message": "demo-cli: crates.io lacks it; this run creates it with the registry token",
    });
    assert!(
        report["proof"].as_array().unwrap().contains(&notice),
        "{}",
        report["proof"]
    );
    assert_eq!(count(&world.github, "PATCH").await, 1);
}

/// A standalone package at `0.0.0` that crates.io lacks joins the plan under
/// `--new`: it is uploaded with the registry token, and the draft
/// Release publishes last and creates tag `v0.0.0`.
#[tokio::test]
async fn new_run_plans_a_new_standalone_package() {
    let world = World::new().await;
    world.fake_cargo();
    standalone(world.repo(), &[("demo", "0.0.0")]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    on_crates_io(&world, "demo", "0.0.0", false).await;
    new_release(&world, "v0.0.0", &head).await;
    let output = release(
        &world,
        &[
            ("VERCTL_REGISTRY_TOKEN", "r3gistry"),
            ("GITHUB_TOKEN", "gh-token"),
        ],
        &["--new"],
    );
    let report = report(&output);
    assert_eq!(world.token_calls(), ["demo r3gistry"]);
    assert_eq!(report["packages"][0]["name"], "demo");
    assert_eq!(
        report["packages"][0]["note"],
        "created with the registry token"
    );
    let draft = body_of(&world.github, "POST", "/repos/acme/demo/releases").await;
    assert_eq!(draft["tag_name"], "v0.0.0", "{draft}");
    assert_eq!(draft["draft"], true, "{draft}");
    let published = body_of(&world.github, "PATCH", "/repos/acme/demo/releases/1").await;
    assert_eq!(published["draft"], false, "{published}");
    assert_eq!(count(&world.github, "POST").await, 1);
}

/// A `0.0.0` crates.io lists, with no tag, is planned without the flag: no
/// upload runs, and the run publishes its draft, which creates the tag.
#[tokio::test]
async fn a_listed_zero_without_a_tag_is_planned() {
    let world = World::new().await;
    world.fake_cargo();
    standalone(world.repo(), &[("demo", "0.0.0")]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    on_crates_io(&world, "demo", "0.0.0", true).await;
    new_release(&world, "v0.0.0", &head).await;
    let output = release(&world, &[("GITHUB_TOKEN", "gh-token")], &[]);
    let report = report(&output);
    assert_eq!(report["packages"][0]["name"], "demo");
    assert_eq!(report["packages"][0]["note"], "already");
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    let draft = body_of(&world.github, "POST", "/repos/acme/demo/releases").await;
    assert_eq!(draft["tag_name"], "v0.0.0", "{draft}");
    assert_eq!(count(&world.github, "PATCH").await, 1);
}

/// Without `--new`, a crate crates.io lacks fails the proof before the
/// draft: nothing is uploaded and GitHub takes no write.
#[tokio::test]
async fn without_new_a_missing_crate_still_fails_the_proof() {
    let world = World::new().await;
    world.fake_cargo();
    workspace(world.repo(), &["demo-core", "demo-cli"]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    known_on_crates_io(&world, "demo-core", "0.7.0").await;
    absent(&world, "demo-cli").await;
    new_release(&world, "v0.7.0", &head).await;
    let output = release(
        &world,
        &[
            ("VERCTL_REGISTRY_TOKEN", "r3gistry"),
            ("GITHUB_TOKEN", "gh-token"),
        ],
        &[],
    );
    assert!(!output.status.success(), "{}", printed(&output));
    let text = printed(&output);
    assert!(text.contains("the release proof failed"), "{text}");
    assert!(text.contains("demo-cli: missing from crates.io"), "{text}");
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(count(&world.github, "POST").await, 0);
    assert_eq!(count(&world.github, "PATCH").await, 0);
}

/// `--new` without `VERCTL_REGISTRY_TOKEN` fails before any request,
/// naming the variable and the workflow secret, whether or not a trusted
/// token is set.
#[tokio::test]
async fn new_needs_the_registry_token() {
    let world = World::new().await;
    world.fake_cargo();
    standalone(world.repo(), &[("demo", "0.0.0")]);
    absent(&world, "demo").await;
    for envs in [
        &[][..],
        &[("VERCTL_TRUSTED_PUBLISHING_TOKEN", "trust3d")][..],
    ] {
        let output = release(&world, envs, &["--new"]);
        assert!(!output.status.success(), "{}", printed(&output));
        let text = printed(&output);
        assert!(text.contains("VERCTL_REGISTRY_TOKEN"), "{text}");
        assert!(text.contains("`registry-token`"), "{text}");
    }
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(received(&world.api).await, no_requests());
    assert_eq!(received(&world.github).await, no_requests());
}

/// With `--new`, the `trust` proof passes a crate crates.io lacks with a
/// notice to add its entry once the run creates it, where it fails a missing
/// crate as `Missing` otherwise.
#[tokio::test]
async fn new_run_passes_the_trust_proof_for_a_crate_it_creates() {
    let world = World::new().await;
    world.fake_cargo();
    standalone(world.repo(), &[("demo", "0.0.0")]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    on_crates_io(&world, "demo", "0.0.0", false).await;
    new_release(&world, "v0.0.0", &head).await;
    let output = release(
        &world,
        &[
            ("VERCTL_REGISTRY_TOKEN", "r3gistry"),
            ("VERCTL_OWNER_TOKEN", OWNER_TOKEN),
            ("GITHUB_TOKEN", "gh-token"),
        ],
        &["--new"],
    );
    let report = report(&output);
    let notice = json!({
        "row": "notice",
        "check": "trust",
        "message": "demo: crates.io lacks it; add its entry with `verctl release trust demo` after this run creates it",
    });
    assert!(
        report["proof"].as_array().unwrap().contains(&notice),
        "{}",
        report["proof"]
    );
    assert_eq!(world.token_calls(), ["demo r3gistry"]);
}

/// `release --dry-run --new` plans what `release --new` plans: a
/// standalone `0.0.0` package crates.io lacks, read from the index, with its
/// tag. Without the flag the same dry run plans nothing.
#[tokio::test]
async fn dry_run_new_plans_what_the_run_plans() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.0.0")]);
    on_crates_io(&world, "demo", "0.0.0", false).await;
    let output = release(
        &world,
        &[("VERCTL_REGISTRY_TOKEN", "r3gistry")],
        &["--dry-run", "--new"],
    );
    let planned = report(&output);
    assert_eq!(planned["outcome"], "released");
    assert_eq!(planned["packages"][0]["name"], "demo");
    assert_eq!(planned["releases"], json!(["would create v0.0.0"]));
    let notice = json!({
        "row": "notice",
        "check": "registry",
        "message": "demo: crates.io lacks it; this run creates it with the registry token",
    });
    assert!(
        planned["proof"].as_array().unwrap().contains(&notice),
        "{}",
        planned["proof"]
    );
    assert!(
        received(&world.index)
            .await
            .contains(&("GET".to_owned(), "/de/mo/demo".to_owned()))
    );
    let plain = report(&release(&world, &[], &["--dry-run"]));
    assert_eq!(plain["outcome"], "nothing-to-release");
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(received(&world.github).await, no_requests());
}

/// Under `--dry-run`, `--new` needs no token: the dry run plans as the
/// real run would, and a notice names `VERCTL_REGISTRY_TOKEN` when it is
/// unset. Only a real run fails without it.
#[tokio::test]
async fn dry_run_new_needs_no_token() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.0.0")]);
    on_crates_io(&world, "demo", "0.0.0", false).await;
    let output = release(&world, &[], &["--dry-run", "--new"]);
    let planned = report(&output);
    assert_eq!(planned["packages"][0]["name"], "demo");
    let notices = planned["notices"].as_array().unwrap();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        notices[0]
            .as_str()
            .unwrap()
            .contains("VERCTL_REGISTRY_TOKEN"),
        "{notices:?}"
    );
    let real = release(&world, &[], &["--new"]);
    assert!(!real.status.success(), "{}", printed(&real));
    assert!(printed(&real).contains("VERCTL_REGISTRY_TOKEN"));
}

/// `release bootstrap` is `release --new` then `release trust`: it creates the
/// missing member with the registry token, publishes the release, and adds the
/// entry of the crate it created and of no other.
#[tokio::test]
async fn bootstrap_creates_a_crate_and_trusts_it() {
    let world = World::new().await;
    world.fake_cargo();
    workspace(world.repo(), &["demo-core", "demo-cli"]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    known_on_crates_io(&world, "demo-core", "0.7.0").await;
    on_crates_io(&world, "demo-cli", "0.7.0", false).await;
    entries(
        &world.api,
        "demo-core",
        vec![entry("acme", "demo", "release.yml", 1)],
    )
    .await;
    entries(&world.api, "demo-cli", Vec::new()).await;
    accepts(&world.api, "demo-cli").await;
    new_release(&world, "v0.7.0", &head).await;
    let output = bootstrap(
        &world,
        &[BOTH[0], BOTH[1], ("GITHUB_TOKEN", "gh-token")],
        &[],
    );
    let report = report(&output);
    assert_eq!(
        report["trust"],
        json!([{ "name": "demo-cli", "state": "added" }])
    );
    assert_eq!(
        world.token_calls(),
        ["demo-cli r3gistry", "demo-core r3gistry"]
    );
    let created = report["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "demo-cli")
        .unwrap();
    assert_eq!(created["note"], "created with the registry token");
    assert_eq!(count(&world.api, "POST").await, 1);
    assert_eq!(count(&world.github, "PATCH").await, 1);
}

/// Two crates named in one run are both created, and both get their entry.
#[tokio::test]
async fn bootstrap_creates_two_named_crates_in_one_run() {
    let world = World::new().await;
    world.fake_cargo();
    workspace(world.repo(), &["demo-core", "demo-cli"]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    on_crates_io(&world, "demo-core", "0.7.0", false).await;
    on_crates_io(&world, "demo-cli", "0.7.0", false).await;
    for name in ["demo-core", "demo-cli"] {
        entries(&world.api, name, Vec::new()).await;
        accepts(&world.api, name).await;
    }
    new_release(&world, "v0.7.0", &head).await;
    let output = bootstrap(
        &world,
        &[BOTH[0], BOTH[1], ("GITHUB_TOKEN", "gh-token")],
        &["demo-core", "demo-cli"],
    );
    let report = report(&output);
    let mut trusted: Vec<String> = report["trust"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| format!("{} {}", row["name"], row["state"]))
        .collect();
    trusted.sort();
    assert_eq!(
        trusted,
        ["\"demo-cli\" \"added\"", "\"demo-core\" \"added\""]
    );
    assert_eq!(count(&world.api, "POST").await, 2);
}

/// Naming a crate allows only that crate: another one crates.io lacks fails
/// the proof as it does without `--new`, and nothing is uploaded.
#[tokio::test]
async fn bootstrap_creates_only_the_crates_it_names() {
    let world = World::new().await;
    world.fake_cargo();
    workspace(world.repo(), &["demo-core", "demo-cli"]);
    let head = common::git(world.repo(), &["rev-parse", "HEAD"]);
    absent(&world, "demo-core").await;
    on_crates_io(&world, "demo-cli", "0.7.0", false).await;
    new_release(&world, "v0.7.0", &head).await;
    let output = bootstrap(
        &world,
        &[BOTH[0], BOTH[1], ("GITHUB_TOKEN", "gh-token")],
        &["demo-cli"],
    );
    assert!(!output.status.success(), "{}", printed(&output));
    let text = printed(&output);
    assert!(text.contains("the release proof failed"), "{text}");
    assert!(text.contains("demo-core: missing from crates.io"), "{text}");
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(count(&world.github, "POST").await, 0);
}

/// A name no crate of the repository has fails before any request, listing
/// the crates it has.
#[tokio::test]
async fn bootstrap_refuses_an_unknown_name() {
    let world = World::new().await;
    world.fake_cargo();
    workspace(world.repo(), &["demo-core", "demo-cli"]);
    let output = bootstrap(&world, &BOTH, &["nosuch"]);
    assert!(!output.status.success(), "{}", printed(&output));
    let text = printed(&output);
    assert!(text.contains("no crate named nosuch"), "{text}");
    assert!(text.contains("demo-core"), "{text}");
    assert!(text.contains("demo-cli"), "{text}");
    assert_eq!(received(&world.api).await, no_requests());
    assert_eq!(received(&world.github).await, no_requests());
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
}

/// A real `release bootstrap` checks both tokens before any write, and names
/// each missing one.
#[tokio::test]
async fn bootstrap_needs_both_tokens() {
    let world = World::new().await;
    world.fake_cargo();
    standalone(world.repo(), &[("demo", "0.0.0")]);
    absent(&world, "demo").await;
    for (present, missing) in [(BOTH[1], BOTH[0].0), (BOTH[0], BOTH[1].0)] {
        let output = bootstrap(&world, &[present], &[]);
        assert!(!output.status.success(), "{}", printed(&output));
        assert!(printed(&output).contains(missing), "{}", printed(&output));
    }
    let output = bootstrap(&world, &[], &[]);
    let text = printed(&output);
    assert!(text.contains(BOTH[0].0), "{text}");
    assert!(text.contains(BOTH[1].0), "{text}");
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(received(&world.api).await, no_requests());
}

/// `release bootstrap --dry-run` needs neither token. It prints what both
/// steps would do, the release and the entry of each crate it would create,
/// and writes nothing.
#[tokio::test]
async fn bootstrap_dry_run_needs_no_token_and_lists_both_steps() {
    let world = World::new().await;
    world.fake_cargo();
    standalone(world.repo(), &[("demo", "0.0.0")]);
    on_crates_io(&world, "demo", "0.0.0", false).await;
    let output = bootstrap(&world, &[], &["--dry-run"]);
    let planned = report(&output);
    assert_eq!(planned["releases"], json!(["would create v0.0.0"]));
    assert_eq!(
        planned["trust"],
        json!([{ "name": "demo", "state": "would-add" }])
    );
    let notices = planned["notices"].as_array().unwrap();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        notices[0]
            .as_str()
            .unwrap()
            .contains("VERCTL_REGISTRY_TOKEN")
    );
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(count(&world.api, "POST").await, 0);
    assert_eq!(received(&world.github).await, no_requests());
}

/// `--bootstrap` is no option of `release`, and the reusable workflow takes
/// `publish-new` and no `bootstrap` input: the old name is not kept.
#[tokio::test]
async fn the_bootstrap_flag_and_input_are_gone() {
    let world = World::new().await;
    standalone(world.repo(), &[("demo", "0.0.0")]);
    let output = release(&world, &[], &["--bootstrap"]);
    assert_eq!(output.status.code(), Some(2), "{}", printed(&output));
    let workflow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/release.yml"),
    )
    .unwrap();
    assert!(workflow.contains("\n      publish-new:\n"), "{workflow}");
    assert!(workflow.contains("VERCTL_NEW: ${{ inputs.publish-new }}"));
    assert!(!workflow.contains("\n      bootstrap:\n"));
    assert!(!workflow.contains("VERCTL_BOOTSTRAP"));
}
