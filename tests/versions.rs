#![allow(clippy::unwrap_used, clippy::expect_used)]

use git2::{Repository, Signature};
use indoc::{formatdoc, indoc};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use verctl::config::Config;
use verctl::versions::{self, Skip};

fn commit_tree(repo: &Repository, message: &str) -> git2::Oid {
    let sig = Signature::now("t", "t@example.com").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tid = index.write_tree().unwrap();
    let tree = repo.find_tree(tid).unwrap();
    let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    let parents: Vec<&git2::Commit<'_>> = parent.as_ref().into_iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .unwrap()
}

fn write_crate(root: &Path, version: &str) {
    fs::write(
        root.join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .unwrap();
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

fn repo_with_origin_main(version: &str) -> (TempDir, Repository) {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    write_crate(dir.path(), version);
    let oid = commit_tree(&repo, "init");
    repo.remote(
        "origin",
        "https://github.com/victor-software-house/verctl.git",
    )
    .unwrap();
    repo.reference("refs/remotes/origin/main", oid, true, "test")
        .unwrap();
    (dir, repo)
}

fn load(root: &Path) -> Config {
    Config::load(&root.join("verctl.toml")).unwrap()
}

fn check(root: &Path, skip: Skip) -> anyhow::Result<versions::VersionReport> {
    versions::require_with(root, &load(root), skip, &versions::stock_candidates())
}

/// Isolate from the host Actions event. Version PR CI sets
/// `GITHUB_EVENT_PATH` to a labeled payload; leaking that into these
/// processes would exempt a hand-edit. Callers that need a var set it
/// after this helper.
fn is_host_actions_key(key: &str) -> bool {
    key == "CI" || key.starts_with("GITHUB_")
}

/// Everything the child may keep. Split from the environment it reads so the
/// rule is testable against a planted one: `std::env` cannot be given a
/// non-Unicode key without `set_var`, and `unsafe_code = "forbid"` rules that
/// out. `vars()` would panic on such a key, so this never decodes one — a key
/// we cannot read is not a key we are stripping.
fn inherited(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> impl Iterator<Item = (OsString, OsString)> {
    vars.into_iter()
        .filter(|(key, _)| !key.to_str().is_some_and(is_host_actions_key))
}

fn check_cmd(root: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"));
    cmd.env_clear()
        .envs(inherited(std::env::vars_os()))
        .current_dir(root)
        .args(["check", "--versions", "--color", "never"]);
    cmd
}

#[test]
fn matching_version_is_ok() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn hand_edited_version_fails() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let err = check(dir.path(), Skip::None).unwrap_err();
    assert!(
        format!("{err:#}").contains("demo 1.0.0 -> 1.0.1"),
        "{err:#}"
    );
}

#[test]
fn fragment_only_change_still_matches() {
    let (dir, _) = repo_with_origin_main("1.0.0");
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
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    check(dir.path(), Skip::VersionBranch).unwrap();
}

#[test]
fn ci_is_exempt() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    check(dir.path(), Skip::Ci).unwrap();
}

#[test]
fn new_package_not_on_default_is_ok() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    fs::write(
        dir.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
            [[packages]]
            name = "fresh"
            path = "crates/fresh/Cargo.toml"
        "#},
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
    Repository::init(dir.path()).unwrap();
    write_crate(dir.path(), "1.0.0");
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn bun_package_json_hand_edit_fails() {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    fs::write(
        dir.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "@org/pkg"
            path = "package.json"
        "#},
    )
    .unwrap();
    fs::write(
        dir.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "0.1.0" }
        "#},
    )
    .unwrap();
    let oid = commit_tree(&repo, "init");
    repo.remote(
        "origin",
        "https://github.com/victor-software-house/verctl.git",
    )
    .unwrap();
    repo.reference("refs/remotes/origin/main", oid, true, "test")
        .unwrap();
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
fn cli_hand_edit_fails_and_prints_table() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = check_cmd(dir.path()).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("demo"), "{stdout}");
    assert!(stdout.contains("1.0.0"), "{stdout}");
    assert!(stdout.contains("1.0.1"), "{stdout}");
    assert!(stderr.contains("fragment"), "{stderr}");
}

/// End-to-end through the real `Command`, so it covers `check_cmd` itself and
/// not just the rule. It only bites where host keys exist — which is CI, the
/// place that broke. `actions_keys_are_dropped_and_unreadable_keys_survive`
/// carries the rule everywhere else.
#[test]
fn check_cmd_hands_the_child_no_actions_keys() {
    let dir = TempDir::new().unwrap();
    let cmd = check_cmd(dir.path());
    let keys: Vec<String> = cmd
        .get_envs()
        .map(|(key, _)| key.to_string_lossy().into_owned())
        .collect();
    assert!(!keys.is_empty(), "the parent environment is forwarded");
    let leaked: Vec<&String> = keys.iter().filter(|key| is_host_actions_key(key)).collect();
    assert!(leaked.is_empty(), "{leaked:?} reached the child");
    assert!(
        keys.iter().any(|key| key == "PATH"),
        "PATH must survive or the binary cannot run: {keys:?}"
    );
}

/// The test above passes for the wrong reason on a host with no `GITHUB_*`
/// set: a leak would only surface in CI. Plant the environment instead, with
/// a key that is not valid UTF-8, so both halves of the rule are real here.
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
    let (dir, _) = repo_with_origin_main("1.0.0");
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
    let (dir, repo) = repo_with_origin_main("1.0.0");
    let first = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature", &first, false).unwrap();
    write_crate(dir.path(), "1.1.0");
    let released = commit_tree(&repo, "release");
    assert_ne!(released, first.id());
    repo.reference("refs/remotes/origin/main", released, true, "test")
        .unwrap();
    repo.set_head("refs/heads/feature").unwrap();
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .unwrap();
    assert_eq!(
        repo.head().unwrap().peel_to_commit().unwrap().id(),
        first.id()
    );
    check(dir.path(), Skip::None).unwrap();
}

#[test]
fn nested_config_maps_onto_the_git_tree() {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    fs::create_dir_all(dir.path().join("crates/demo")).unwrap();
    fs::write(
        dir.path().join("crates/demo/verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .unwrap();
    fs::write(
        dir.path().join("crates/demo/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .unwrap();
    let oid = commit_tree(&repo, "init");
    repo.remote(
        "origin",
        "https://github.com/victor-software-house/verctl.git",
    )
    .unwrap();
    repo.reference("refs/remotes/origin/main", oid, true, "test")
        .unwrap();
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
        &Config::load(&dir.path().join("crates/demo/verctl.toml")).unwrap(),
        Skip::None,
        &versions::stock_candidates(),
    )
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("demo 1.0.0 -> 1.0.1"),
        "{err:#}"
    );
}

#[test]
fn current_branch_reads_version_packages() {
    let (dir, repo) = repo_with_origin_main("1.0.0");
    let oid = repo.head().unwrap().peel_to_commit().unwrap().id();
    repo.reference("refs/heads/version-packages", oid, true, "test")
        .unwrap();
    repo.set_head("refs/heads/version-packages").unwrap();
    assert_eq!(
        verctl::git::current_branch(dir.path()).as_deref(),
        Some("version-packages")
    );
}

#[test]
fn pull_request_head_ref_is_not_an_exemption() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_HEAD_REF", "version-packages")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn pull_request_version_label_is_exempt() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let event = dir.path().join("event.json");
    fs::write(
        &event,
        r#"{"pull_request":{"labels":[{"name":"verctl:version"}]}}"#,
    )
    .unwrap();
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_EVENT_PATH", &event)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("version-pr"), "{stdout}");
}

#[test]
fn issue_event_label_is_not_an_exemption() {
    let (dir, _) = repo_with_origin_main("1.0.0");
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
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let output = check_cmd(dir.path())
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_REF_NAME", "version-packages")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn configured_version_label_is_the_exemption() {
    let (dir, _) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    fs::write(
        dir.path().join("verctl.toml"),
        indoc! {r#"
            [prepare]
            version_label = "ship-it"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .unwrap();
    let default_event = dir.path().join("default.json");
    fs::write(
        &default_event,
        r#"{"pull_request":{"labels":[{"name":"verctl:version"}]}}"#,
    )
    .unwrap();
    let defaulted = check_cmd(dir.path())
        .env("GITHUB_EVENT_PATH", &default_event)
        .output()
        .unwrap();
    assert!(
        !defaulted.status.success(),
        "default label must not exempt when version_label is ship-it"
    );
    let event = dir.path().join("event.json");
    fs::write(
        &event,
        r#"{"pull_request":{"labels":[{"name":"ship-it"}]}}"#,
    )
    .unwrap();
    let output = check_cmd(dir.path())
        .env("GITHUB_EVENT_PATH", &event)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("version-pr"), "{stdout}");
}

#[test]
fn version_packages_branch_cli_is_exempt() {
    let (dir, repo) = repo_with_origin_main("1.0.0");
    write_crate(dir.path(), "1.0.1");
    let oid = repo.head().unwrap().peel_to_commit().unwrap().id();
    repo.reference("refs/heads/version-packages", oid, true, "test")
        .unwrap();
    repo.set_head("refs/heads/version-packages").unwrap();
    let output = check_cmd(dir.path()).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("version-pr"), "{stdout}");
}
