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
use wiremock::matchers::{body_json, body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// The owner's token, which the entry requests carry as `Authorization`.
const OWNER_TOKEN: &str = "t0ken";

/// Both tokens a real `release bootstrap` needs.
const BOTH: [(&str, &str); 2] = [
    ("VERCTL_REGISTRY_TOKEN", "r3gistry"),
    ("VERCTL_OWNER_TOKEN", OWNER_TOKEN),
];

/// What crates.io prints when it refuses an upload for its rate limit, with a
/// time long past.
const RATE_LIMITED: &str = "the remote server responded with an error (status 429 Too Many Requests): Please try again after Fri, 13 Nov 2026 19:00:00 GMT or email help@crates.io";

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

    /// A `cargo` first on `PATH`: `metadata` goes to the real one, and every
    /// other call is logged, marks the crate it names as published, and, with
    /// `limit_once`, first fails once with crates.io's rate limit.
    fn fake_cargo(&self, limit_once: bool) {
        let real = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let tools = self.tools.path().display();
        let limit = if limit_once {
            formatdoc! {r#"
                if [ ! -e "{tools}/$crate.limited" ]; then
                    touch "{tools}/$crate.limited"
                    echo "{RATE_LIMITED}" >&2
                    exit 101
                fi
            "#}
        } else {
            String::new()
        };
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
            {limit}touch "{tools}/$crate.up"
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

/// GitHub has no tag `tag` and takes its creation at `sha`.
async fn takes_tag(world: &World, tag: &str, sha: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/acme/demo/git/ref/tags/{tag}")))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "message": "Not Found",
            "documentation_url": "https://docs.github.com",
        })))
        .mount(&world.github)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/demo/git/refs"))
        .and(body_partial_json(
            json!({ "ref": format!("refs/tags/{tag}"), "sha": sha }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "ref": format!("refs/tags/{tag}"),
            "node_id": "R",
            "url": format!("https://api.github.com/repos/acme/demo/git/refs/tags/{tag}"),
            "object": {
                "type": "commit",
                "sha": sha,
                "url": format!("https://api.github.com/repos/acme/demo/git/commits/{sha}"),
            },
        })))
        .mount(&world.github)
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
        .envs(envs.iter().copied())
        .args(["--format", "json", "--color", "never", "release", step])
        .args(extra)
        .args(["--crates-io-api", &world.api.uri()]);
    if step == "bootstrap" {
        command.args(["--crates-io-index", &world.index.uri()]);
    }
    command.output().unwrap()
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

/// Of a workspace's two crates, one listed and one missing, only the missing
/// one is published, with `cargo publish --package`. It gets no tag, because
/// a member's tag belongs to the group's release, and trust adds its entry.
#[tokio::test]
async fn bootstrap_publishes_only_the_missing_crate() {
    let world = World::new().await;
    world.fake_cargo(false);
    workspace(world.repo(), &["demo-core", "demo-cli"]);
    on_crates_io(&world, "demo-core", "0.7.0", true).await;
    on_crates_io(&world, "demo-cli", "0.7.0", false).await;
    entries(&world.api, "demo-cli", Vec::new()).await;
    accepts(&world.api, "demo-cli").await;
    let output = bootstrap(&world, &BOTH, &[]);
    assert_eq!(
        rows(&output),
        [
            json!({ "name": "demo-cli", "version": "0.7.0", "state": "published" }),
            json!({ "name": "demo-cli", "version": "0.7.0", "state": "added" }),
        ]
    );
    let calls = world.cargo_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls[0].starts_with("publish --locked --manifest-path "),
        "{calls:?}"
    );
    assert!(calls[0].ends_with("--package demo-cli"), "{calls:?}");
    assert_eq!(received(&world.github).await, no_requests());
    assert_eq!(count(&world.api, "POST").await, 1);
}

/// A standalone package is tagged at the default branch's tip after its
/// upload, then trust adds its entry.
#[tokio::test]
async fn bootstrap_tags_a_standalone_package() {
    let world = World::new().await;
    world.fake_cargo(false);
    standalone(world.repo(), &[("demo", "0.0.0")]);
    let tip = common::git(world.repo(), &["rev-parse", "HEAD"]);
    on_crates_io(&world, "demo", "0.0.0", false).await;
    entries(&world.api, "demo", Vec::new()).await;
    accepts(&world.api, "demo").await;
    takes_tag(&world, "v0.0.0", &tip).await;
    let output = bootstrap(
        &world,
        &[BOTH[0], BOTH[1], ("GITHUB_TOKEN", "gh-token")],
        &[],
    );
    assert_eq!(
        rows(&output),
        [
            json!({ "name": "demo", "version": "0.0.0", "state": "published" }),
            json!({ "name": "demo", "version": "0.0.0", "state": "tagged", "tag": "v0.0.0" }),
            json!({ "name": "demo", "version": "0.0.0", "state": "added" }),
        ]
    );
    assert_eq!(world.cargo_calls().len(), 1);
    assert_eq!(count(&world.github, "POST").await, 1);
}

/// A crate named on the command line that crates.io lists is reported as
/// already published, nothing is uploaded, and the run exits zero.
#[tokio::test]
async fn bootstrap_skips_a_named_listed_crate() {
    let world = World::new().await;
    world.fake_cargo(false);
    standalone(world.repo(), &[("demo", "0.1.0")]);
    on_crates_io(&world, "demo", "0.1.0", true).await;
    let output = bootstrap(&world, &BOTH, &["demo"]);
    assert_eq!(
        rows(&output),
        [json!({ "name": "demo", "version": "0.1.0", "state": "already-published" })]
    );
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
}

/// The index lists a crate that the owners API still answers 404 for, as on a
/// rerun. It is reported as already published, and still gets its tag and its
/// trusted publisher entry, which the run needs to count as done.
#[tokio::test]
async fn bootstrap_tags_and_trusts_a_crate_the_index_already_lists() {
    let world = World::new().await;
    world.fake_cargo(false);
    standalone(world.repo(), &[("demo", "0.0.0")]);
    let tip = common::git(world.repo(), &["rev-parse", "HEAD"]);
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
        .and(path("/crates/demo/owners"))
        .respond_with(ResponseTemplate::new(404))
        .up_to_n_times(1)
        .mount(&world.api)
        .await;
    Mock::given(method("GET"))
        .and(path("/crates/demo/owners"))
        .respond_with(ResponseTemplate::new(200).set_body_json(owners))
        .mount(&world.api)
        .await;
    Mock::given(method("GET"))
        .and(path("/de/mo/demo"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(json!({ "name": "demo", "vers": "0.0.0" }).to_string()),
        )
        .mount(&world.index)
        .await;
    entries(&world.api, "demo", Vec::new()).await;
    accepts(&world.api, "demo").await;
    takes_tag(&world, "v0.0.0", &tip).await;
    let output = bootstrap(
        &world,
        &[BOTH[0], BOTH[1], ("GITHUB_TOKEN", "gh-token")],
        &[],
    );
    assert_eq!(
        rows(&output),
        [
            json!({ "name": "demo", "version": "0.0.0", "state": "already-published" }),
            json!({ "name": "demo", "version": "0.0.0", "state": "tagged", "tag": "v0.0.0" }),
            json!({ "name": "demo", "version": "0.0.0", "state": "added" }),
        ]
    );
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(count(&world.github, "POST").await, 1);
    assert_eq!(count(&world.api, "POST").await, 1);
}

/// Each token a real run needs is checked before any request: each missing
/// one is named, and nothing is read or uploaded.
#[tokio::test]
async fn bootstrap_needs_both_tokens() {
    let world = World::new().await;
    world.fake_cargo(false);
    standalone(world.repo(), &[("demo", "0.0.0")]);
    absent(&world, "demo").await;
    for (present, missing) in [(BOTH[1], BOTH[0].0), (BOTH[0], BOTH[1].0)] {
        let output = bootstrap(&world, &[present], &[]);
        assert!(!output.status.success(), "{}", printed(&output));
        assert!(printed(&output).contains(missing), "{}", printed(&output));
    }
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(received(&world.api).await, no_requests());
}

/// HEAD one commit behind the default branch fails naming both commits, and
/// uploads nothing.
#[tokio::test]
async fn bootstrap_refuses_head_behind_the_default_branch() {
    let world = World::new().await;
    world.fake_cargo(false);
    standalone(world.repo(), &[("demo", "0.0.0")]);
    absent(&world, "demo").await;
    let behind = common::git(world.repo(), &["rev-parse", "HEAD"]);
    let tip = common::commit_all(world.repo(), "ahead");
    common::git(
        world.repo(),
        &["update-ref", "refs/remotes/origin/main", &tip],
    );
    common::git(world.repo(), &["reset", "-q", "--hard", &behind]);
    let output = bootstrap(&world, &BOTH, &[]);
    assert!(!output.status.success(), "{}", printed(&output));
    let text = printed(&output);
    assert!(text.contains(&behind), "{text}");
    assert!(text.contains(&tip), "{text}");
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
}

/// `--dry-run` needs no token, lists what it would publish and tag, and
/// writes nothing: no upload, no tag, no entry.
#[tokio::test]
async fn bootstrap_dry_run_writes_nothing() {
    let world = World::new().await;
    world.fake_cargo(false);
    standalone(world.repo(), &[("demo", "0.0.0")]);
    on_crates_io(&world, "demo", "0.0.0", false).await;
    let output = bootstrap(&world, &[], &["--dry-run"]);
    assert_eq!(
        rows(&output),
        [
            json!({ "name": "demo", "version": "0.0.0", "state": "would-publish" }),
            json!({ "name": "demo", "version": "0.0.0", "state": "would-tag", "tag": "v0.0.0" }),
        ]
    );
    assert_eq!(world.cargo_calls(), Vec::<String>::new());
    assert_eq!(count(&world.api, "POST").await, 0);
    assert_eq!(received(&world.github).await, no_requests());
}

/// The first upload is stopped by crates.io's rate limit, whose time is past:
/// the run notes the wait, uploads again, and publishes.
#[tokio::test]
async fn bootstrap_waits_out_a_rate_limit() {
    let world = World::new().await;
    world.fake_cargo(true);
    workspace(world.repo(), &["demo-cli"]);
    on_crates_io(&world, "demo-cli", "0.7.0", false).await;
    entries(&world.api, "demo-cli", Vec::new()).await;
    accepts(&world.api, "demo-cli").await;
    let output = bootstrap(&world, &BOTH, &[]);
    assert_eq!(
        rows(&output),
        [
            json!({
                "name": "demo-cli",
                "version": "0.7.0",
                "state": "published",
                "note": "waited until Fri, 13 Nov 2026 19:00:00 GMT",
            }),
            json!({ "name": "demo-cli", "version": "0.7.0", "state": "added" }),
        ]
    );
    assert_eq!(world.cargo_calls().len(), 2);
}
