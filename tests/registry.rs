//! The `registry` and `trust` checks against a recorded crates.io API and a
//! repository whose default branch holds some of the crates.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fixture setup unwraps; a broken fixture fails the test"
)]

mod common;

use std::fs;
use std::path::Path;

use indoc::{formatdoc, indoc};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::task;
use verctl::config::{self, Config};
use verctl::github::{self, Repo};
use verctl::registry_check::{
    self, Crate, Evidence, RegistryReport, RegistryState, TrustReport, TrustState,
};
use verctl::registry_token::OWNER;
use verctl::versions;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The repository [`common::on_default_branch`] names as `origin`.
fn acme_demo() -> Repo {
    github::parse_slug("acme/demo").unwrap()
}

/// A crate manifest at `<dir>/Cargo.toml` that names `name`.
fn write_crate(root: &Path, dir: &str, name: &str, version: &str) {
    let manifest = root.join(dir).join("Cargo.toml");
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(
        manifest,
        formatdoc! {r#"
            [package]
            name = "{name}"
            version = "{version}"
            edition = "2024"
        "#},
    )
    .unwrap();
}

/// [`repository_at`] with each held package at `0.1.0`.
fn repository(top: &str, held: &[&str], added: &[&str]) -> TempDir {
    repository_at(top, held, added, "0.1.0")
}

/// A repository whose default branch holds a package for each of `held`,
/// at `<name>/Cargo.toml` and `held_version`, and whose working tree adds one
/// at `0.0.0` for each of `added`, as a pull request would. `.ctl/ver.yaml`
/// declares them all, after the top-level `top`.
fn repository_at(top: &str, held: &[&str], added: &[&str], held_version: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let packages: String = held
        .iter()
        .chain(added)
        .map(|name| {
            formatdoc! {"
                - name: {name}
                  path: {name}/Cargo.toml
            "}
        })
        .collect();
    common::write_config(
        dir.path(),
        &formatdoc! {"
            {top}
            packages:
            {packages}"},
    );
    for name in held {
        write_crate(dir.path(), name, name, held_version);
    }
    common::on_default_branch(dir.path());
    for name in added {
        write_crate(dir.path(), name, name, "0.0.0");
    }
    dir
}

/// A virtual workspace at `root` whose members are `members`, each at the
/// shared version `0.7.0`.
fn write_workspace(root: &Path, members: &[&str]) {
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
        let member = root.join(name);
        fs::create_dir_all(member.join("src")).unwrap();
        fs::write(member.join("src/lib.rs"), "").unwrap();
        fs::write(
            member.join("Cargo.toml"),
            formatdoc! {r#"
                [package]
                name = "{name}"
                version.workspace = true
                edition.workspace = true
            "#},
        )
        .unwrap();
    }
}

/// The crates of the repository at `root`, placed against `origin/main`.
fn crates_of(root: &Path) -> Vec<Crate> {
    let config = Config::load(&root.join(config::FILE)).unwrap();
    registry_check::crates(&config, root, &versions::stock_candidates()).unwrap()
}

/// crates.io lists `logins` as the user owners of `name`.
async fn owned_by(server: &MockServer, name: &str, logins: &[&str]) {
    let users: Vec<Value> = logins
        .iter()
        .enumerate()
        .map(|(id, login)| {
            json!({
                "id": id,
                "login": login,
                "kind": "user",
                "url": format!("https://github.com/{login}"),
                "name": null,
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path(format!("/crates/{name}/owners")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "users": users })))
        .mount(server)
        .await;
}

/// crates.io does not know `name`.
async fn absent(server: &MockServer, name: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/crates/{name}/owners")))
        .respond_with(ResponseTemplate::new(404))
        .mount(server)
        .await;
}

/// The newest version of `name` has `trustpub_data` as its record.
async fn newest(server: &MockServer, name: &str, trustpub_data: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/crates/{name}/versions")))
        .and(query_param("sort", "date"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "versions": [{ "num": "0.1.0", "trustpub_data": trustpub_data }],
            "meta": { "total": 1, "next_page": null },
        })))
        .mount(server)
        .await;
}

/// `name` has one trusted publisher entry for `acme/demo` per file of
/// `workflows`, served only to the token `t0ken`.
async fn entries(server: &MockServer, name: &str, workflows: &[&str]) {
    let configs: Vec<Value> = workflows
        .iter()
        .enumerate()
        .map(|(id, workflow)| {
            json!({
                "id": id,
                "crate": name,
                "repository_owner": "acme",
                "repository_owner_id": 9,
                "repository_name": "demo",
                "workflow_filename": workflow,
                "environment": null,
                "created_at": "2026-11-13T00:00:00Z",
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/trusted_publishing/github_configs"))
        .and(query_param("crate", name))
        .and(header("authorization", "t0ken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "github_configs": configs,
            "meta": { "total": workflows.len(), "next_page": null },
        })))
        .mount(server)
        .await;
}

/// crates.io refuses `name`'s entries to the token: its user does not own the
/// crate.
async fn refused(server: &MockServer, name: &str) {
    Mock::given(method("GET"))
        .and(path("/trusted_publishing/github_configs"))
        .and(query_param("crate", name))
        .and(header("authorization", "t0ken"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "errors": [{ "detail": "You are not an owner of this crate" }],
        })))
        .mount(server)
        .await;
}

/// `registry` over `crates` against `server`, off the async runtime as the
/// command runs it.
async fn registry(server: &MockServer, crates: Vec<Crate>) -> RegistryReport {
    let api = server.uri();
    task::spawn_blocking(move || registry_check::prove_registry(&crates, &api, &acme_demo(), false))
        .await
        .unwrap()
        .unwrap()
}

/// `trust` with the token `t0ken` over `crates` against `server`, naming
/// `workflow`.
async fn trust(server: &MockServer, crates: Vec<Crate>, workflow: &str) -> TrustReport {
    let api = server.uri();
    let workflow = workflow.to_owned();
    task::spawn_blocking(move || {
        registry_check::prove_trust(&crates, &api, "t0ken", &acme_demo(), &workflow, false)
    })
    .await
    .unwrap()
    .unwrap()
}

/// Every crate is ready: each exists and lists `any-victor`. The newest
/// version's trusted publishing is evidence only, so a token upload passes.
#[tokio::test]
async fn every_crate_is_ready() {
    let repo = repository("owners: [any-victor]", &["demo", "demo-core"], &[]);
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["Any-Victor", "alice"]).await;
    owned_by(&server, "demo-core", &["any-victor"]).await;
    newest(
        &server,
        "demo",
        json!({ "provider": "github", "repository": "Acme/Demo", "run_id": "1", "sha": "aaa" }),
    )
    .await;
    newest(&server, "demo-core", Value::Null).await;
    let report = registry(&server, crates_of(repo.path())).await;
    assert_eq!(report.failures(), Vec::<String>::new());
    assert_eq!(report.notices(), Vec::<String>::new());
    let trusted: Vec<&Evidence> = report
        .crates
        .iter()
        .map(|row| match &row.state {
            RegistryState::Present { trusted, .. } => trusted,
            RegistryState::Missing
            | RegistryState::New
            | RegistryState::Creates
            | RegistryState::NotAtZero { .. } => {
                panic!("{row:?}")
            }
        })
        .collect();
    assert_eq!(
        trusted,
        [
            &Evidence::ThisRepository {
                repository: "Acme/Demo".into()
            },
            &Evidence::Untrusted
        ]
    );
    assert_eq!(trusted[0].label(), "trusted from Acme/Demo");
    assert_eq!(trusted[1].label(), "not trusted");
}

/// An owner is missing: the failure names the crate, the missing login, and
/// the owners crates.io found.
#[tokio::test]
async fn an_owner_is_missing() {
    let repo = repository("owners: [any-victor]", &["demo"], &[]);
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["alice"]).await;
    newest(&server, "demo", Value::Null).await;
    let report = registry(&server, crates_of(repo.path())).await;
    let failures = report.failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("demo: lacks owner any-victor on crates.io, which lists alice"),
        "{failures:?}"
    );
}

/// Without `owners`, the owner proof is skipped with a notice, and the crate
/// passes whoever owns it.
#[tokio::test]
async fn no_owners_skips_the_owner_proof() {
    let repo = repository("", &["demo"], &[]);
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["alice"]).await;
    newest(&server, "demo", Value::Null).await;
    let report = registry(&server, crates_of(repo.path())).await;
    assert_eq!(report.failures(), Vec::<String>::new());
    assert_eq!(
        report.notices(),
        ["demo: owner proof skipped; set owners in .ctl/ver.yaml to prove them"]
    );
}

/// A workspace whose default branch holds member `demo`, and whose working
/// tree adds member `demo-extra` at `0.0.0`, as a pull request would.
fn workspace_adding_a_member() -> TempDir {
    let dir = TempDir::new().unwrap();
    common::write_config(
        dir.path(),
        indoc! {"
            owners: [any-victor]
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    let root = |members: &str| {
        formatdoc! {r#"
            [workspace]
            resolver = "3"
            members = [{members}]

            [workspace.package]
            version = "0.1.0"
            edition = "2024"
        "#}
    };
    fs::write(dir.path().join("Cargo.toml"), root(r#""demo""#)).unwrap();
    write_crate(dir.path(), "demo", "demo", "0.1.0");
    fs::create_dir_all(dir.path().join("demo/src")).unwrap();
    fs::write(dir.path().join("demo/src/lib.rs"), "").unwrap();
    common::on_default_branch(dir.path());
    fs::write(
        dir.path().join("Cargo.toml"),
        root(r#""demo", "demo-extra""#),
    )
    .unwrap();
    write_crate(dir.path(), "demo-extra", "demo-extra", "0.0.0");
    fs::create_dir_all(dir.path().join("demo-extra/src")).unwrap();
    fs::write(dir.path().join("demo-extra/src/lib.rs"), "").unwrap();
    dir
}

/// A new crate on a pull request: a workspace counts each member, and the
/// member the default branch lacks passes, with a notice that it needs its
/// bootstrap once it lands.
#[tokio::test]
async fn a_new_crate_on_a_pull_request() {
    let repo = workspace_adding_a_member();
    let crates = crates_of(repo.path());
    let placed: Vec<(&str, bool)> = crates
        .iter()
        .map(|listed| (listed.name.as_str(), listed.held))
        .collect();
    assert_eq!(placed, [("demo", true), ("demo-extra", false)]);
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["any-victor"]).await;
    newest(&server, "demo", Value::Null).await;
    absent(&server, "demo-extra").await;
    let report = registry(&server, crates).await;
    assert_eq!(report.failures(), Vec::<String>::new());
    assert_eq!(
        report.notices(),
        [
            "demo-extra: not on crates.io yet; it needs `verctl release bootstrap demo-extra` once it lands on the default branch"
        ]
    );
}

/// A new crate with owners set: with `owners: [any-victor]`, a crate neither
/// the default branch nor crates.io holds runs no owner proof, and gets the
/// bootstrap notice.
#[tokio::test]
async fn a_new_crate_with_owners_set() {
    let repo = repository("owners: [any-victor]", &["demo"], &["demo-extra"]);
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["any-victor"]).await;
    newest(&server, "demo", Value::Null).await;
    absent(&server, "demo-extra").await;
    let report = registry(&server, crates_of(repo.path())).await;
    assert_eq!(report.failures(), Vec::<String>::new());
    assert_eq!(report.crates[1].state, RegistryState::New);
    assert!(!report.crates[1].held);
    assert_eq!(report.notices().len(), 1);
}

/// A new crate whose name crates.io holds: the owner proof applies as to any
/// other crate, so `alice`'s crate fails.
#[tokio::test]
async fn a_new_crate_whose_name_crates_io_holds() {
    let repo = repository("owners: [any-victor]", &[], &["demo-extra"]);
    let server = MockServer::start().await;
    owned_by(&server, "demo-extra", &["alice"]).await;
    newest(&server, "demo-extra", Value::Null).await;
    let report = registry(&server, crates_of(repo.path())).await;
    assert!(!report.crates[0].held);
    let failures = report.failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0]
            .starts_with("demo-extra: lacks owner any-victor on crates.io, which lists alice"),
        "{failures:?}"
    );
}

/// A crate the default branch holds at `0.0.0` and the registry lacks fails,
/// named as missing from crates.io, with `verctl release bootstrap <name>` as
/// the fix.
#[tokio::test]
async fn a_crate_the_default_branch_holds_and_the_registry_lacks() {
    let repo = repository_at("owners: [any-victor]", &["demo-extra"], &[], "0.0.0");
    let server = MockServer::start().await;
    absent(&server, "demo-extra").await;
    let report = registry(&server, crates_of(repo.path())).await;
    assert_eq!(report.crates[0].state, RegistryState::Missing);
    assert_eq!(
        report.failures(),
        [
            "demo-extra: missing from crates.io, though the default branch holds it; run `verctl release bootstrap demo-extra` at the default branch's tip"
        ]
    );
}

/// A standalone package neither crates.io nor the default branch has, at a
/// version other than `0.0.0`, fails naming the crate, the version, and
/// `0.0.0`, and says `verctl release bootstrap` publishes it there.
#[tokio::test]
async fn registry_fails_a_new_standalone_crate_off_zero() {
    let repo = repository(
        indoc! {r#"
            owners: [any-victor]
            tags:
              template: "{name}@{version}"
        "#},
        &["demo"],
        &["demo-extra"],
    );
    write_crate(repo.path(), "demo-extra", "demo-extra", "0.2.0");
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["any-victor"]).await;
    newest(&server, "demo", Value::Null).await;
    absent(&server, "demo-extra").await;
    let report = registry(&server, crates_of(repo.path())).await;
    assert!(!report.crates[1].held);
    assert_eq!(
        report.crates[1].state,
        RegistryState::NotAtZero {
            version: "0.2.0".into()
        }
    );
    assert_eq!(
        report.failures(),
        [
            "demo-extra: not on crates.io, and its version 0.2.0 is not 0.0.0; a never-published standalone package starts at 0.0.0, which `verctl release bootstrap demo-extra` publishes"
        ]
    );
}

/// A workspace member first publishes at the group's version, so the `0.0.0`
/// rule leaves it out: one the default branch holds fails as missing, one it
/// does not hold passes with the bootstrap notice, both at `0.7.0`.
#[tokio::test]
async fn registry_exempts_a_workspace_member() {
    let dir = TempDir::new().unwrap();
    common::write_config(
        dir.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    write_workspace(dir.path(), &["demo-core"]);
    common::on_default_branch(dir.path());
    write_workspace(dir.path(), &["demo-core", "demo-cli"]);
    let crates = crates_of(dir.path());
    assert_eq!(crates.len(), 2);
    for listed in &crates {
        assert_eq!(listed.version, "0.7.0");
        assert!(!listed.standalone);
    }
    let server = MockServer::start().await;
    absent(&server, "demo-core").await;
    absent(&server, "demo-cli").await;
    let report = registry(&server, crates).await;
    let state = |name: &str| {
        &report
            .crates
            .iter()
            .find(|row| row.name == name)
            .unwrap()
            .state
    };
    assert_eq!(state("demo-core"), &RegistryState::Missing);
    assert_eq!(state("demo-cli"), &RegistryState::New);
    assert_eq!(
        report.failures(),
        [
            "demo-core: missing from crates.io, though the default branch holds it; run `verctl release bootstrap demo-core` at the default branch's tip"
        ]
    );
}

/// A rerun repeats the proof: after the bootstrap, the same check repeats
/// every request and passes.
#[tokio::test]
async fn a_rerun_repeats_the_proof() {
    let repo = repository_at("owners: [any-victor]", &["demo-extra"], &[], "0.0.0");
    let server = MockServer::start().await;
    absent(&server, "demo-extra").await;
    assert_eq!(
        registry(&server, crates_of(repo.path()))
            .await
            .failures()
            .len(),
        1
    );
    server.reset().await;
    owned_by(&server, "demo-extra", &["any-victor"]).await;
    newest(&server, "demo-extra", Value::Null).await;
    let report = registry(&server, crates_of(repo.path())).await;
    assert_eq!(report.failures(), Vec::<String>::new());
}

/// A missing entry: an owner's token reads the entries, and none names the
/// default workflow file `release.yml`, so the failure names the crate and
/// that file. An entry that names it passes.
#[tokio::test]
async fn trust_with_a_missing_entry() {
    let repo = repository("owners: [any-victor]", &["demo", "demo-core"], &[]);
    let config = Config::load(&repo.path().join(config::FILE)).unwrap();
    assert_eq!(config.release_workflow, "release.yml");
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["any-victor"]).await;
    owned_by(&server, "demo-core", &["any-victor"]).await;
    entries(&server, "demo", &["self-release.yml"]).await;
    entries(&server, "demo-core", &["release.yml"]).await;
    let report = trust(&server, crates_of(repo.path()), &config.release_workflow).await;
    let TrustReport::Set { crates, .. } = &report else {
        panic!("{report:?}");
    };
    assert!(
        matches!(&crates[0].state, TrustState::Untrusted { entries } if entries.len() == 1),
        "{crates:?}"
    );
    assert_eq!(crates[1].state, TrustState::Trusted);
    assert_eq!(
        report.failures(),
        [
            "demo: lacks a trusted publisher for release.yml in acme/demo; add one on crates.io that names this repository and workflow file"
        ]
    );
}

/// A crate whose entries crates.io refuses to the token fails alone: the run
/// returns a report, the next crate is still read and passes, and the failure
/// names only the refused crate and crates.io's reason.
#[tokio::test]
async fn trust_with_a_refused_crate_still_checks_the_next() {
    let repo = repository("owners: [any-victor]", &["demo", "demo-core"], &[]);
    let config = Config::load(&repo.path().join(config::FILE)).unwrap();
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["any-victor"]).await;
    owned_by(&server, "demo-core", &["any-victor"]).await;
    refused(&server, "demo").await;
    entries(&server, "demo-core", &["release.yml"]).await;
    let report = trust(&server, crates_of(repo.path()), &config.release_workflow).await;
    let TrustReport::Set { crates, .. } = &report else {
        panic!("{report:?}");
    };
    assert_eq!(
        crates[0].state,
        TrustState::Refused {
            detail: "You are not an owner of this crate".into()
        }
    );
    assert_eq!(crates[1].state, TrustState::Trusted);
    assert_eq!(
        report.failures(),
        [
            "demo: crates.io refused to list its trusted publishers (You are not an owner of this crate); the token's user must be an owner of the crate"
        ]
    );
}

/// A caller of another name: with `release_workflow: self-release.yml`, the
/// entry naming that file passes.
#[tokio::test]
async fn trust_looks_for_the_release_workflow() {
    let repo = repository("release_workflow: self-release.yml", &["demo"], &[]);
    let config = Config::load(&repo.path().join(config::FILE)).unwrap();
    let server = MockServer::start().await;
    owned_by(&server, "demo", &["any-victor"]).await;
    entries(&server, "demo", &["self-release.yml"]).await;
    let report = trust(&server, crates_of(repo.path()), &config.release_workflow).await;
    assert_eq!(report.failures(), Vec::<String>::new());
}

/// No token: `trust` fails naming `VERCTL_OWNER_TOKEN`, and sends no
/// request to `/api/v1/trusted_publishing/github_configs`, or anywhere.
#[tokio::test]
async fn trust_with_no_token_sends_no_request() {
    let repo = repository("owners: [any-victor]", &["demo"], &[]);
    let config = Config::load(&repo.path().join(config::FILE)).unwrap();
    let server = MockServer::start().await;
    entries(&server, "demo", &["release.yml"]).await;
    let api = server.uri();
    let root = repo.path().to_path_buf();
    let report = task::spawn_blocking(move || registry_check::trust(&config, &root, &api, None))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(report, TrustReport::Unset { variable: OWNER });
    let failures = report.failures();
    assert_eq!(failures.len(), 1);
    assert!(
        failures[0].starts_with("VERCTL_OWNER_TOKEN is unset"),
        "{failures:?}"
    );
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .all(|request| request.url.path() != "/trusted_publishing/github_configs"),
        "{requests:?}"
    );
    assert!(requests.is_empty(), "{requests:?}");
}
