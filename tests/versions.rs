//! The merge-base version guard: a manifest version must match the default
//! branch unless exempt.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fixture setup unwraps; a broken fixture fails the test"
)]

mod common;

use std::ffi::OsString;
use std::fs;
use std::path::Path;

use indoc::{formatdoc, indoc};
use tempfile::TempDir;
use verctl::config::Config;
use verctl::versions::{self, Skip};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ORIGIN: &str = "https://github.com/victor-software-house/verctl.git";

fn track_origin_main(dir: &Path, sha: &str) {
    common::git(dir, &["remote", "add", "origin", ORIGIN]);
    common::git(dir, &["update-ref", "refs/remotes/origin/main", sha]);
}

fn write_crate(root: &Path, version: &str) {
    common::write_config(
        root,
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(
        root.join("Cargo.toml"),
        formatdoc! {r#"
            [package]
            name = "demo"
            version = "{version}"
        "#},
    )
    .unwrap();
}

fn repo_with_origin_main(version: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    common::init(dir.path());
    write_crate(dir.path(), version);
    let sha = common::commit_all(dir.path(), "init");
    track_origin_main(dir.path(), &sha);
    dir
}

fn load(root: &Path) -> Config {
    Config::load(&root.join(verctl::config::FILE)).unwrap()
}

/// The check of a pull request into `main`: its base is the default branch.
fn check(root: &Path, skip: Skip) -> anyhow::Result<versions::VersionReport> {
    let stock = versions::stock_candidates();
    versions::require_with(root, &load(root), skip, &stock, &stock)
}

/// Isolate from the host Actions event. Version PR CI sets
/// `GITHUB_EVENT_PATH` and a token; leaking them into these processes would
/// exempt a hand-edit. Callers that need a var set it
/// after this helper.
fn is_host_actions_key(key: &str) -> bool {
    key == "CI" || key.starts_with("GITHUB_")
}

/// Everything the child may keep: no host Actions key, and nothing
/// [`common::is_cleared`] strips from every verctl child, since `env_clear`
/// followed by `envs` would otherwise put those back. Split from the
/// environment it reads so the rule is testable against a planted one:
/// `std::env` cannot be given a non-Unicode key without `set_var`, and
/// `unsafe_code = "forbid"` rules that out. `vars()` would panic on such a key,
/// so this never decodes one — a key we cannot read is not a key we are
/// stripping.
fn inherited(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> impl Iterator<Item = (OsString, OsString)> {
    vars.into_iter().filter(|(key, _)| {
        !key.to_str().is_some_and(is_host_actions_key) && !common::is_cleared(key)
    })
}

fn check_cmd(root: &Path) -> std::process::Command {
    let mut cmd = common::verctl();
    cmd.env_clear()
        .envs(inherited(std::env::vars_os()))
        .current_dir(root)
        .args(["check", "--versions", "--color", "never"]);
    cmd
}

#[test]
fn matching_version_is_ok() {
    let dir = repo_with_origin_main("1.0.0");
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn hand_edited_version_fails() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let err = check(dir.path(), Skip::None).unwrap_err();
    assert!(
        format!("{err:#}").contains("demo 1.0.0 -> 1.0.1"),
        "{err:#}"
    );
}

#[test]
fn fragment_only_change_still_matches() {
    let dir = repo_with_origin_main("1.0.0");
    fs::create_dir_all(dir.path().join(".changeset")).unwrap();
    fs::write(
        dir.path().join(".changeset/note.md"),
        indoc! {"
            ---
            demo: patch
            ---

            A fragment. Version is still 1.0.0.
        "},
    )
    .unwrap();
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn version_packages_branch_is_exempt() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    check(dir.path(), Skip::VersionBranch).unwrap();
}

#[test]
fn ci_is_exempt() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    check(dir.path(), Skip::Ci).unwrap();
}

#[test]
fn new_package_not_on_default_is_ok() {
    let dir = repo_with_origin_main("1.0.0");
    fs::write(
        dir.path().join(verctl::config::FILE),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
              - name: fresh
                path: crates/fresh/Cargo.toml
        "},
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("crates/fresh")).unwrap();
    fs::write(
        dir.path().join("crates/fresh/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "fresh"
            version = "0.0.1"
        "#},
    )
    .unwrap();
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn no_origin_is_ok() {
    let dir = TempDir::new().unwrap();
    common::init(dir.path());
    write_crate(dir.path(), "1.0.0");
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn bun_package_json_hand_edit_fails() {
    let dir = TempDir::new().unwrap();
    common::init(dir.path());
    common::write_config(
        dir.path(),
        indoc! {r#"
            packages:
              - name: "@org/pkg"
                path: package.json
        "#},
    );
    fs::write(
        dir.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "0.1.0" }
        "#},
    )
    .unwrap();
    let sha = common::commit_all(dir.path(), "init");
    track_origin_main(dir.path(), &sha);
    fs::write(
        dir.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "0.2.0" }
        "#},
    )
    .unwrap();
    let err = check(dir.path(), Skip::None).unwrap_err();
    assert!(
        format!("{err:#}").contains("@org/pkg 0.1.0 -> 0.2.0"),
        "{err:#}"
    );
}

#[test]
fn cli_hand_edit_fails_with_one_stderr_report() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = check_cmd(dir.path()).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert_eq!(output.stdout, &[] as &[u8]);
    assert!(stderr.contains("demo"), "{stderr}");
    assert!(stderr.contains("1.0.0"), "{stderr}");
    assert!(stderr.contains("1.0.1"), "{stderr}");
    assert!(stderr.contains("fragment"), "{stderr}");
}

/// Asserting against the real environment would prove nothing on a host with
/// no `GITHUB_*` set. Plant one instead, including a key that is not valid
/// UTF-8, so both halves of the rule hold everywhere. That `check_cmd` applies
/// the rule is what the CLI tests above already prove: remove its `env_clear`
/// and four of them fail wherever host keys exist.
#[cfg(unix)]
#[test]
fn actions_keys_are_dropped_and_unreadable_keys_survive() {
    use std::os::unix::ffi::OsStringExt;

    let opaque = OsString::from_vec(vec![0xff, 0xfe]);
    let planted = vec![
        (OsString::from("PATH"), OsString::from("/usr/bin")),
        (OsString::from("CI"), OsString::from("true")),
        (
            OsString::from("GITHUB_EVENT_PATH"),
            OsString::from("/event.json"),
        ),
        (OsString::from("GITHUB_BASE_REF"), OsString::from("main")),
        (
            OsString::from("VERCTL_CONFIG"),
            OsString::from("elsewhere.yaml"),
        ),
        (OsString::from("FORCE_COLOR"), OsString::from("3")),
        (opaque.clone(), OsString::from("not utf-8")),
    ];
    let kept: Vec<OsString> = inherited(planted).map(|(key, _)| key).collect();
    assert_eq!(kept, [OsString::from("PATH"), opaque]);
}

#[test]
fn host_actions_keys_are_not_inherited() {
    assert!(is_host_actions_key("CI"));
    assert!(is_host_actions_key("GITHUB_EVENT_PATH"));
    assert!(is_host_actions_key("GITHUB_BASE_REF"));
    assert!(is_host_actions_key("GITHUB_REPOSITORY"));
    assert!(is_host_actions_key("GITHUB_HEAD_REF"));
    assert!(is_host_actions_key("GITHUB_REF_NAME"));
    assert!(!is_host_actions_key("PATH"));
    assert!(!is_host_actions_key("HOME"));
}

#[test]
fn ci_env_does_not_skip() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = check_cmd(dir.path())
        .env("CI", "true")
        .env("GITHUB_ACTIONS", "true")
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn behind_released_main_without_local_edit_is_ok() {
    let dir = repo_with_origin_main("1.0.0");
    let first = common::git(dir.path(), &["rev-parse", "HEAD"]);
    common::git(dir.path(), &["branch", "feature", &first]);
    write_crate(dir.path(), "1.1.0");
    let released = common::commit_all(dir.path(), "release");
    assert_ne!(released, first);
    common::git(
        dir.path(),
        &["update-ref", "refs/remotes/origin/main", &released],
    );
    common::git(dir.path(), &["checkout", "-q", "-f", "feature"]);
    assert_eq!(common::git(dir.path(), &["rev-parse", "HEAD"]), first);
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn nested_config_maps_onto_the_git_tree() {
    let dir = TempDir::new().unwrap();
    common::init(dir.path());
    fs::create_dir_all(dir.path().join("crates/demo")).unwrap();
    common::write_config(
        &dir.path().join("crates/demo"),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(
        dir.path().join("crates/demo/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .unwrap();
    let sha = common::commit_all(dir.path(), "init");
    track_origin_main(dir.path(), &sha);
    fs::write(
        dir.path().join("crates/demo/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.1"
        "#},
    )
    .unwrap();
    let err = versions::require_with(
        &dir.path().join("crates/demo"),
        &Config::load(&dir.path().join("crates/demo").join(verctl::config::FILE)).unwrap(),
        Skip::None,
        &versions::stock_candidates(),
        &versions::stock_candidates(),
    )
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("demo 1.0.0 -> 1.0.1"),
        "{err:#}"
    );
}

#[test]
fn current_branch_reads_the_version_branch() {
    let dir = repo_with_origin_main("1.0.0");
    common::git(dir.path(), &["checkout", "-q", "-b", "verctl/release/main"]);
    assert_eq!(
        verctl::git::current_branch(dir.path()).as_deref(),
        Some("verctl/release/main")
    );
}

#[test]
fn pull_request_head_ref_is_not_an_exemption() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_HEAD_REF", "verctl/release/main")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A GitHub API that answers pull request 7 with its head repository, head
/// branch, and base.
async fn pull_api(head_repo: &str, head_ref: &str, base_ref: &str) -> MockServer {
    let server = MockServer::start().await;
    let body = serde_json::json!({
        "id": 1,
        "number": 7,
        "url": "https://api.github.com/repos/owner/demo/pulls/7",
        "locked": false,
        "labels": [{"id": 1, "node_id": "L", "url": "https://api.github.com/l", "name": "verctl:version", "color": "ffffff", "default": false}],
        "head": {
            "ref": head_ref,
            "sha": "0000000000000000000000000000000000000001",
            "repo": {"id": 2, "name": "demo", "full_name": head_repo, "url": format!("https://api.github.com/repos/{head_repo}")}
        },
        "base": {"ref": base_ref, "sha": "0000000000000000000000000000000000000002"}
    });
    Mock::given(method("GET"))
        .and(path("/repos/owner/demo/pulls/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

/// `check --versions` as the CI job of pull request 7 runs it, with the pull
/// request read from `server`. The payload claims a Version PR, which the
/// check must not trust.
fn pull_request_cmd(root: &Path, server: &MockServer) -> std::process::Command {
    let event = root.join("event.json");
    fs::write(
        &event,
        r#"{"pull_request":{"number":7,"labels":[{"name":"verctl:version"}],"head":{"ref":"verctl/release/main"},"base":{"ref":"main"}}}"#,
    )
    .unwrap();
    let mut cmd = check_cmd(root);
    cmd.env("GITHUB_ACTIONS", "true")
        .env("GITHUB_EVENT_PATH", &event)
        .env("GITHUB_REPOSITORY", "owner/demo")
        .env("GITHUB_API_URL", server.uri())
        .env("GITHUB_TOKEN", "token");
    cmd
}

/// Whether `check --versions` passes for pull request 7 as `server` describes
/// it, with a hand-edited version.
fn pull_request_passes(server: &MockServer) -> bool {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = pull_request_cmd(dir.path(), server).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.success(),
        stdout.contains("version-pr"),
        "{stdout}{stderr}"
    );
    output.status.success()
}

/// An ordinary pull request that changes no version reads no API, so it
/// passes without a token.
#[test]
fn a_pull_request_that_changes_no_version_reads_no_api() {
    let dir = repo_with_origin_main("1.0.0");
    let event = dir.path().join("event.json");
    fs::write(&event, r#"{"pull_request":{"number":7}}"#).unwrap();
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_EVENT_PATH", &event)
        .env("GITHUB_REPOSITORY", "owner/demo")
        .env("GITHUB_API_URL", "http://127.0.0.1:9")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!stdout.contains("version-pr"), "{stdout}");
}

/// A merge queue run skips a changed version without reading the API: the
/// Version PR's own entry, and every entry behind it, carry its versions.
#[test]
fn a_merge_queue_run_skips_a_changed_version() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let event = dir.path().join("event.json");
    fs::write(
        &event,
        r#"{"action":"checks_requested","merge_group":{"head_ref":"refs/heads/gh-readonly-queue/main/pr-7-0000000000000000000000000000000000000001","base_ref":"refs/heads/main"}}"#,
    )
    .unwrap();
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_EVENT_PATH", &event)
        .env("GITHUB_REPOSITORY", "owner/demo")
        .env("GITHUB_API_URL", "http://127.0.0.1:9")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("merge-queue"), "{stdout}");
}

#[tokio::test]
async fn the_version_pr_identity_is_exempt() {
    let server = pull_api("owner/demo", "verctl/release/main", "main").await;
    assert!(pull_request_passes(&server));
}

#[tokio::test]
async fn a_version_branch_from_a_fork_is_not_an_exemption() {
    let server = pull_api("fork/demo", "verctl/release/main", "main").await;
    assert!(!pull_request_passes(&server));
}

/// A base changed after opening no longer matches the head branch, whatever
/// the event payload said.
#[tokio::test]
async fn a_retargeted_version_pr_is_not_an_exemption() {
    let server = pull_api("owner/demo", "verctl/release/main", "next").await;
    assert!(!pull_request_passes(&server));
}

/// Labels are never read: the Version PR label on an ordinary branch exempts
/// nothing.
#[tokio::test]
async fn a_labeled_ordinary_pull_request_is_not_an_exemption() {
    let server = pull_api("owner/demo", "feat/bump", "main").await;
    assert!(!pull_request_passes(&server));
}

#[test]
fn issue_event_label_is_not_an_exemption() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let event = dir.path().join("event.json");
    fs::write(
        &event,
        r#"{"issue":{"labels":[{"name":"verctl:version"}]}}"#,
    )
    .unwrap();
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_EVENT_PATH", &event)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn github_ref_name_is_not_an_exemption() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_REF_NAME", "verctl/release/main")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository whose `main` released 1.0.1 after `next` branched from it at
/// 1.0.0, with both branches tracked on `origin` and HEAD on `next`.
fn repo_with_next() -> TempDir {
    let dir = repo_with_origin_main("1.0.0");
    common::git(dir.path(), &["checkout", "-q", "-b", "next"]);
    fs::write(dir.path().join("next.txt"), "next\n").unwrap();
    let next = common::commit_all(dir.path(), "next work");
    common::git(
        dir.path(),
        &["update-ref", "refs/remotes/origin/next", &next],
    );
    common::git(dir.path(), &["checkout", "-q", "main"]);
    write_crate(dir.path(), "1.0.1");
    let main = common::commit_all(dir.path(), "release 1.0.1");
    common::git(
        dir.path(),
        &["update-ref", "refs/remotes/origin/main", &main],
    );
    common::git(dir.path(), &["checkout", "-q", "next"]);
    dir
}

/// The check of a pull request into `next`.
fn check_next(root: &Path) -> anyhow::Result<versions::VersionReport> {
    versions::require_with(
        root,
        &load(root),
        Skip::None,
        &verctl::git::default_branch_candidates_from(Some("next")),
        &versions::stock_candidates(),
    )
}

/// Merging `main` into `next` brings 1.0.1 along, which `main` released.
#[test]
fn merging_main_into_next_is_not_a_hand_edit() {
    let dir = repo_with_next();
    common::git(dir.path(), &["merge", "-q", "--no-edit", "main"]);
    let report = check_next(dir.path()).unwrap();
    assert_eq!(report.rows[0].remote.as_deref(), Some("1.0.0"));
    assert_eq!(report.rows[0].released.as_deref(), Some("1.0.1"));
}

/// Typing the version `main` released, without its history, is a hand edit.
#[test]
fn a_hand_edit_on_next_fails() {
    let dir = repo_with_next();
    write_crate(dir.path(), "1.0.1");
    let err = check_next(dir.path()).unwrap_err();
    assert!(
        format!("{err:#}").contains("demo 1.0.0 -> 1.0.1"),
        "{err:#}"
    );
}

/// Actions leaves `origin/HEAD` unset, so a default branch named `trunk`
/// resolves from the pull request's base for both merge-bases.
#[test]
fn a_pull_request_into_trunk_resolves_both_merge_bases() {
    let dir = TempDir::new().unwrap();
    common::init(dir.path());
    write_crate(dir.path(), "1.0.0");
    let sha = common::commit_all(dir.path(), "init");
    common::git(dir.path(), &["remote", "add", "origin", ORIGIN]);
    common::git(
        dir.path(),
        &["update-ref", "refs/remotes/origin/trunk", &sha],
    );
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_BASE_REF", "trunk")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn version_branch_cli_is_exempt() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    common::git(dir.path(), &["checkout", "-q", "-b", "verctl/release/main"]);
    let output = check_cmd(dir.path()).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("version-pr"), "{stdout}");
}

/// The Changesets branch name is no longer a Version PR: a hand edit there
/// fails like anywhere else.
#[test]
fn old_version_packages_branch_is_not_exempt() {
    let dir = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    common::git(dir.path(), &["checkout", "-q", "-b", "version-packages"]);
    let output = check_cmd(dir.path()).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("demo"), "{stderr}");
    assert!(stderr.contains("1.0.1"), "{stderr}");
}
