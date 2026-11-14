//! The release freeze of `check registry`, through the binary, against a
//! recorded GitHub and crates.io.
//! A pull request or merge group fails while the default branch holds an
//! unreleased version, and any other event only reports it.
//! The `verctl: unfreeze` label passes the freeze alone.

mod common;

use std::path::Path;
use std::process::{Command, Output};
use std::{env, fs};

use indoc::formatdoc;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The workflow file the freeze waits for: `release_workflow`'s default.
const RUNS: &str = "/repos/acme/demo/actions/workflows/release.yml/runs";

fn manifest(version: &str) -> String {
    formatdoc! {r#"
        [package]
        name = "demo"
        version = "{version}"
    "#}
}

/// A repository whose default branch holds `demo` at `version`, with `top`
/// before the packages in `.ctl/ver.yaml`. Returns the directory and the tip
/// of `main`.
fn repository(top: &str, version: &str) -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    common::write_config(
        dir.path(),
        &formatdoc! {"
            {top}packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(dir.path().join("Cargo.toml"), manifest(version)).unwrap();
    common::on_default_branch(dir.path());
    let tip = common::git(dir.path(), &["rev-parse", "HEAD"]);
    (dir, tip)
}

fn not_found() -> ResponseTemplate {
    ResponseTemplate::new(404).set_body_json(json!({
        "message": "Not Found",
        "documentation_url": "https://docs.github.com",
    }))
}

/// What GitHub holds for the release of `demo` `0.4.0`.
enum Release {
    /// No tag and no Release.
    Missing,
    /// A published Release whose tag names `head`.
    Published { head: String },
}

/// A GitHub that answers the lookups of tag `v0.4.0` as `release` says.
async fn github_with(release: &Release) -> MockServer {
    let server = MockServer::start().await;
    match release {
        Release::Missing => {
            for route in [
                "/repos/acme/demo/git/ref/tags/v0.4.0",
                "/repos/acme/demo/releases/tags/v0.4.0",
            ] {
                Mock::given(method("GET"))
                    .and(path(route))
                    .respond_with(not_found())
                    .mount(&server)
                    .await;
            }
            Mock::given(method("GET"))
                .and(path("/repos/acme/demo/releases"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
                .mount(&server)
                .await;
        }
        Release::Published { head } => {
            Mock::given(method("GET"))
                .and(path("/repos/acme/demo/git/ref/tags/v0.4.0"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "ref": "refs/tags/v0.4.0",
                    "node_id": "R",
                    "url": "https://api.github.com/repos/acme/demo/git/refs/tags/v0.4.0",
                    "object": {
                        "type": "commit",
                        "sha": head,
                        "url": format!("https://api.github.com/repos/acme/demo/git/commits/{head}"),
                    },
                })))
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/repos/acme/demo/releases/tags/v0.4.0"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id": 1,
                    "html_url": "https://github.com/acme/demo/releases/tag/v0.4.0",
                    "tag_name": "v0.4.0",
                    "draft": false,
                    "immutable": false,
                    "assets": [],
                })))
                .mount(&server)
                .await;
        }
    }
    server
}

/// GitHub lists one run of `release.yml` at `tip` per entry of `statuses`,
/// for the next `times` reads, or for every read with `None`.
async fn runs(server: &MockServer, tip: &str, statuses: &[&str], times: Option<u64>) {
    let listed: Vec<Value> = statuses
        .iter()
        .enumerate()
        .map(|(id, status)| {
            json!({
                "id": id + 1,
                "html_url": format!("https://github.com/acme/demo/actions/runs/{}", id + 1),
                "head_sha": tip,
                "status": status,
            })
        })
        .collect();
    let mock = Mock::given(method("GET"))
        .and(path(RUNS))
        .and(query_param("head_sha", tip))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": listed.len(),
            "workflow_runs": listed,
        })));
    match times {
        Some(times) => mock.up_to_n_times(times).mount(server).await,
        None => mock.mount(server).await,
    }
}

/// A crates.io whose index lists the `listed` versions of `demo`, and whose
/// API lists `owners` as its user owners. Two servers, so a test can tell
/// which one a request reached.
async fn crates_io(listed: &[&str], owners: &[&str]) -> (MockServer, MockServer) {
    let index = MockServer::start().await;
    let lines: Vec<String> = listed
        .iter()
        .map(|version| json!({ "name": "demo", "vers": version }).to_string())
        .collect();
    Mock::given(method("GET"))
        .and(path("/de/mo/demo"))
        .respond_with(ResponseTemplate::new(200).set_body_string(lines.join("\n")))
        .mount(&index)
        .await;
    let api = MockServer::start().await;
    let users: Vec<Value> = owners
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
        .and(path("/crates/demo/owners"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "users": users })))
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/crates/demo/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "versions": [{ "num": "0.3.0", "trustpub_data": null }],
            "meta": { "total": 1, "next_page": null },
        })))
        .mount(&api)
        .await;
    (index, api)
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

/// An Actions event: its name, and the payload at `GITHUB_EVENT_PATH`.
struct Event {
    name: &'static str,
    payload: Value,
}

/// A `pull_request` event for pull request 7, labelled `labels`, whose
/// repository's default branch is `main`.
fn pull_request(labels: &[&str]) -> Event {
    Event {
        name: "pull_request",
        payload: json!({
            "repository": { "default_branch": "main" },
            "pull_request": {
                "number": 7,
                "labels": labels.iter().map(|name| json!({ "name": name })).collect::<Vec<_>>(),
            },
        }),
    }
}

/// A `merge_group` event whose group holds pull request 7.
fn merge_group() -> Event {
    Event {
        name: "merge_group",
        payload: json!({
            "repository": { "default_branch": "main" },
            "merge_group": {
                "head_ref": "refs/heads/gh-readonly-queue/main/pr-7-0123456789abcdef",
            },
        }),
    }
}

/// `check registry` with `args` in `root`, as `event` runs it, against
/// `github` and `crates_io`. The report is laid out wide, so no message wraps
/// inside a word.
fn check(
    root: &Path,
    github: &MockServer,
    (index, api): &(MockServer, MockServer),
    event: &Event,
    args: &[&str],
) -> Output {
    let events = TempDir::new().unwrap();
    let payload = events.path().join("event.json");
    fs::write(&payload, event.payload.to_string()).unwrap();
    off_actions()
        .current_dir(root)
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_API_URL", github.uri())
        .env("GITHUB_EVENT_NAME", event.name)
        .env("GITHUB_EVENT_PATH", payload)
        .env("COLUMNS", "400")
        .args(["--color", "never", "check", "registry"])
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

/// The paths of the requests `server` received, in order.
async fn paths(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| request.url.path().to_owned())
        .collect()
}

const LABEL: &str = "verctl: unfreeze";

/// `main` holds `demo` `0.4.0`, crates.io does not list it, and no release
/// run is active: another pull request fails naming `demo@0.4.0`.
#[tokio::test]
async fn a_pull_request_freezes_on_a_partial_release() {
    let (repo, tip) = repository("", "0.4.0");
    let github = github_with(&Release::Missing).await;
    runs(&github, &tip, &[], None).await;
    let registry = crates_io(&["0.3.0"], &["any-victor"]).await;
    let output = check(repo.path(), &github, &registry, &pull_request(&[]), &[]);
    assert!(!output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains("`demo@0.4.0` is not fully released on the default branch"),
        "{said}"
    );
    assert!(
        said.contains(&format!("label this pull request `{LABEL}`")),
        "{said}"
    );
}

/// A release run at the tip of `main` is in progress on the first read and
/// has ended on the second, and the release is complete by then: the check
/// waits for it and exits zero.
#[tokio::test]
async fn a_running_release_is_waited_for() {
    let (repo, tip) = repository("", "0.4.0");
    let github = github_with(&Release::Published { head: tip.clone() }).await;
    runs(&github, &tip, &["in_progress"], Some(1)).await;
    runs(&github, &tip, &["completed"], None).await;
    let registry = crates_io(&["0.4.0"], &["any-victor"]).await;
    let output = check(
        repo.path(),
        &github,
        &registry,
        &pull_request(&[]),
        &["--release-wait", "2"],
    );
    assert!(output.status.success(), "{output:?}");
    let seen = paths(&github).await;
    assert_eq!(
        seen.iter().filter(|seen| *seen == RUNS).count(),
        2,
        "{seen:?}"
    );
}

/// The release run is still in progress when the limit ends: the check fails
/// naming the run and the limit.
#[tokio::test]
async fn the_wait_runs_out() {
    let (repo, tip) = repository("", "0.4.0");
    let github = github_with(&Release::Missing).await;
    runs(&github, &tip, &["in_progress"], None).await;
    let registry = crates_io(&["0.3.0"], &["any-victor"]).await;
    let output = check(
        repo.path(),
        &github,
        &registry,
        &pull_request(&[]),
        &["--release-wait", "1"],
    );
    assert!(!output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains("https://github.com/acme/demo/actions/runs/1"),
        "{said}"
    );
    assert!(
        said.contains("the 1-second limit of --release-wait"),
        "{said}"
    );
}

/// The label passes the freeze with a notice that names it and the pending
/// release, and a missing owner still fails the check.
#[tokio::test]
async fn the_unfreeze_label_passes_only_the_freeze() {
    let (repo, tip) = repository("", "0.4.0");
    let github = github_with(&Release::Missing).await;
    runs(&github, &tip, &[], None).await;
    let registry = crates_io(&["0.3.0"], &["any-victor"]).await;
    let output = check(
        repo.path(),
        &github,
        &registry,
        &pull_request(&[LABEL]),
        &[],
    );
    assert!(output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains(&format!(
            "`demo@0.4.0` is not fully released on the default branch; the label `{LABEL}` passes the freeze"
        )),
        "{said}"
    );

    let (repo, tip) = repository("owners: [any-victor]\n", "0.4.0");
    let github = github_with(&Release::Missing).await;
    runs(&github, &tip, &[], None).await;
    let registry = crates_io(&["0.3.0"], &["alice"]).await;
    let output = check(
        repo.path(),
        &github,
        &registry,
        &pull_request(&[LABEL]),
        &[],
    );
    assert!(!output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains("demo: lacks owner any-victor on crates.io, which lists alice"),
        "{said}"
    );
    assert!(
        !said.contains("finish or repair that release first"),
        "{said}"
    );
}

/// A merge group names its pull request in its head ref, and the labels come
/// from the API.
#[tokio::test]
async fn a_merge_group_reads_the_label_of_its_pull_request() {
    let (repo, tip) = repository("", "0.4.0");
    let github = github_with(&Release::Missing).await;
    runs(&github, &tip, &[], None).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/pulls/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "number": 7,
            "labels": [{ "id": 1, "name": LABEL, "color": "ededed", "default": false }],
        })))
        .mount(&github)
        .await;
    let registry = crates_io(&["0.3.0"], &["any-victor"]).await;
    let output = check(repo.path(), &github, &registry, &merge_group(), &[]);
    assert!(output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains(&format!("the label `{LABEL}` passes the freeze")),
        "{said}"
    );
    let seen = paths(&github).await;
    assert!(
        seen.iter().any(|seen| seen == "/repos/acme/demo/pulls/7"),
        "{seen:?}"
    );
}

/// The Version PR holds `demo` `0.4.0` while `main` holds `0.3.0`, and
/// crates.io lacks `0.4.0`: the pending release is not one the default branch
/// holds, so the pull request does not freeze on it.
#[tokio::test]
async fn the_version_pr_does_not_freeze_on_its_own_versions() {
    let (repo, tip) = repository("", "0.3.0");
    fs::write(repo.path().join("Cargo.toml"), manifest("0.4.0")).unwrap();
    common::commit_all(repo.path(), "chore: version");
    let github = github_with(&Release::Missing).await;
    runs(&github, &tip, &[], None).await;
    let registry = crates_io(&["0.3.0"], &["any-victor"]).await;
    let output = check(repo.path(), &github, &registry, &pull_request(&[]), &[]);
    assert!(output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(!said.contains("not fully released"), "{said}");
}

/// On a push the same pending release is a notice: the check exits zero, and
/// it waits for no run.
#[tokio::test]
async fn a_push_only_reports_the_pending_release() {
    let (repo, tip) = repository("", "0.4.0");
    let github = github_with(&Release::Missing).await;
    runs(&github, &tip, &[], None).await;
    let registry = crates_io(&["0.3.0"], &["any-victor"]).await;
    let push = Event {
        name: "push",
        payload: json!({ "repository": { "default_branch": "main" } }),
    };
    let output = check(repo.path(), &github, &registry, &push, &[]);
    assert!(output.status.success(), "{output:?}");
    let said = printed(&output);
    assert!(
        said.contains("`demo@0.4.0` is not fully released on the default branch"),
        "{said}"
    );
    assert!(said.contains("this run only reports it"), "{said}");
    let seen = paths(&github).await;
    assert!(!seen.iter().any(|seen| seen == RUNS), "{seen:?}");
}
