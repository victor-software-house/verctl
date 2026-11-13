//! The `status`, `check`, and `version` commands as run through the binary,
//! including JSON output.

mod common;

use std::fs;
use std::path::Path;
use std::process::Output;

use common::demo_root;
use indoc::{formatdoc, indoc};
use tempfile::TempDir;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn status_lists_mixed_packages() {
    let root = TempDir::new().expect("tempdir");
    common::write_demo_package(root.path());
    let dir = root.path().join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("restore.md"),
        indoc! {r#"
            ---
            forkctl: patch
            "@scope/pkg": minor
            ---

            Restore.
        "#},
    )
    .expect("write");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["status", "--color", "never", "-d"])
        .arg(&dir)
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("forkctl"), "{stdout}");
    assert!(stdout.contains("@scope/pkg"), "{stdout}");
    assert!(stdout.contains("pending"), "{stdout}");
    assert!(stdout.contains("minor"), "{stdout}");
    assert!(stdout.contains('│') || stdout.contains('|'), "{stdout}");
}

#[test]
fn status_json_uses_the_same_report_without_ansi() {
    let root = TempDir::new().expect("tempdir");
    common::write_demo_package(root.path());
    let dir = root.path().join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("change.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Patch.
        "},
    )
    .expect("write");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["status", "--format", "json", "--color", "always", "-d"])
        .arg(&dir)
        .output()
        .expect("spawn");
    assert!(output.status.success());
    assert_eq!(output.stderr, Vec::<u8>::new());
    assert!(!output.stdout.contains(&0x1b));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["pending"], 1);
    assert_eq!(value["max"], "patch");
    assert_eq!(value["fragments"][0]["packages"][0]["name"], "demo");
    assert!(value["unreleased"].is_null(), "{value}");
    assert_eq!(value["unreleased_unchecked"], "no GITHUB_TOKEN or GH_TOKEN");
}

/// A token is set but the check cannot run, here because the clone has no
/// GitHub `origin`. `status` still reports, with the failure as the reason;
/// only the plan for `--github-output` fails on it.
#[test]
fn a_check_that_fails_leaves_unreleased_unchecked() {
    let root = TempDir::new().expect("tempdir");
    common::write_demo_package(root.path());
    common::init(root.path());
    common::commit_all(root.path(), "demo 1.0.0");
    let status = |extra: &[&str]| {
        common::verctl()
            .current_dir(root.path())
            .env_remove("GITHUB_REPOSITORY")
            .env("GITHUB_TOKEN", "test")
            .env("GITHUB_API_URL", "http://127.0.0.1:9")
            .args(["status", "--format", "json"])
            .args(extra)
            .output()
            .expect("spawn")
    };
    let output = status(&[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert!(value["unreleased"].is_null(), "{value}");
    assert!(
        value["unreleased_unchecked"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()),
        "{value}"
    );
    let out = root.path().join("out");
    let planned = status(&["--github-output", out.to_str().expect("utf-8")]);
    assert!(!planned.status.success(), "{planned:?}");
}

/// The case `actions/checkout` produces: a depth-1 clone whose version
/// commit lies past the shallow boundary, with a token set. The plan names
/// HEAD and walks no history, so the boundary stops nothing: `status` reaches
/// the tag lookup, and here only the unreachable GitHub leaves the releases
/// unchecked.
#[test]
fn a_depth_1_clone_plans_without_a_history_walk() {
    let work = TempDir::new().expect("tempdir");
    let source = work.path().join("source");
    fs::create_dir_all(&source).expect("dir");
    common::write_demo_package(&source);
    common::init(&source);
    common::commit_all(&source, "demo 1.0.0");
    fs::write(source.join("README.md"), "Demo.\n").expect("readme");
    common::commit_all(&source, "docs: add a readme");
    let url = format!("file://{}", source.display());
    common::git(
        work.path(),
        &["clone", "-q", "--depth", "1", url.as_str(), "clone"],
    );
    let clone = work.path().join("clone");
    common::git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/demo.git",
        ],
    );
    let status = |extra: &[&str]| {
        common::verctl()
            .current_dir(&clone)
            .env("GITHUB_TOKEN", "test")
            .env("GITHUB_API_URL", "http://127.0.0.1:9")
            .arg("status")
            .args(extra)
            .output()
            .expect("spawn")
    };
    let output = status(&["--format", "json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert!(value["unreleased"].is_null(), "{value}");
    assert!(
        value["unreleased_unchecked"]
            .as_str()
            .is_some_and(|reason| reason.contains("read tag v1.0.0")
                && !reason.contains("the clone is shallow")),
        "{value}"
    );
}

/// `verctl status --format json` with a token and a repository at
/// `demo@1.0.0` whose `origin` is GitHub. GitHub answers the lookup of tag
/// `v1.0.0` with `answer`.
async fn status_with_tag_lookup(answer: ResponseTemplate) -> Output {
    let root = TempDir::new().expect("tempdir");
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
        "},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .expect("cargo");
    common::init(root.path());
    common::commit_all(root.path(), "demo 1.0.0");
    common::git(
        root.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/demo.git",
        ],
    );
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/git/ref/tags/v1.0.0"))
        .respond_with(answer)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    common::verctl()
        .current_dir(root.path())
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_API_URL", server.uri())
        .args(["status", "--format", "json"])
        .output()
        .expect("spawn")
}

/// A tag ref GitHub would return for `v1.0.0` naming `object`.
fn tag_ref(kind: &str, sha: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "ref": "refs/tags/v1.0.0",
        "node_id": "R",
        "url": "https://api.github.com/repos/acme/demo/git/refs/tags/v1.0.0",
        "object": {
            "type": kind,
            "sha": sha,
            "url": format!("https://api.github.com/repos/acme/demo/git/{kind}s/{sha}")
        }
    }))
}

/// A release tag that GitHub reports on a commit HEAD does not hold at that
/// version is a wrong answer. It is not a lookup that could not run, so
/// `status` fails on it even without `--github-output`.
#[tokio::test]
async fn a_misplaced_release_tag_fails_status() {
    let output = status_with_tag_lookup(tag_ref(
        "commit",
        "1111111111111111111111111111111111111111",
    ))
    .await;
    assert!(!output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tag v1.0.0 names"), "{stdout}");
}

/// A release tag on a tree is a wrong answer too.
#[tokio::test]
async fn a_release_tag_on_a_tree_fails_status() {
    let output =
        status_with_tag_lookup(tag_ref("tree", "2222222222222222222222222222222222222222")).await;
    assert!(!output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tag v1.0.0 names Tree"), "{stdout}");
}

/// A 401 is GitHub's answer to a bad token, and a 403 without a spent rate
/// limit is a refusal. The operator has to fix either, so `status` fails on
/// them rather than reporting the check as not run.
#[tokio::test]
async fn a_rejected_token_fails_status() {
    for status in [401, 403] {
        let output = status_with_tag_lookup(ResponseTemplate::new(status).set_body_json(
            serde_json::json!({
                "message": "Bad credentials",
                "documentation_url": "https://docs.github.com/rest"
            }),
        ))
        .await;
        assert!(!output.status.success(), "{status}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("read tag v1.0.0"), "{status}: {stdout}");
    }
}

/// A 5xx, even an HTML page from a proxy, is no answer. A spent rate limit is
/// none either: a 429, or a 403 with `x-ratelimit-remaining: 0`. In both
/// cases `status` reports the check as not run.
#[tokio::test]
async fn a_github_that_cannot_answer_now_leaves_unreleased_unchecked() {
    for answer in [
        ResponseTemplate::new(502)
            .set_body_raw("<html><body>Bad gateway</body></html>", "text/html"),
        ResponseTemplate::new(429),
        ResponseTemplate::new(403).insert_header("x-ratelimit-remaining", "0"),
    ] {
        let output = status_with_tag_lookup(answer).await;
        assert!(output.status.success(), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
        assert!(value["unreleased"].is_null(), "{value}");
        let reason = value["unreleased_unchecked"].as_str().expect("reason");
        assert!(reason.starts_with("read tag v1.0.0"), "{reason}");
    }
}

#[test]
fn quiet_suppresses_only_successful_pretty_status() {
    let root = TempDir::new().expect("tempdir");
    common::write_demo_package(root.path());
    let output = common::verctl()
        .current_dir(root.path())
        .args(["status", "--quiet", "-d"])
        .arg(root.path().join("missing"))
        .output()
        .expect("spawn");
    assert!(output.status.success());
    assert_eq!(output.stdout, Vec::<u8>::new());
    assert_eq!(output.stderr, Vec::<u8>::new());
}

#[test]
fn json_errors_use_stdout_and_one_envelope() {
    let root = TempDir::new().expect("tempdir");
    let dir = root.path().join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("bad.md"), "nope").expect("write");
    let output = common::verctl()
        .args(["check", "--format", "json", "-d"])
        .arg(&dir)
        .output()
        .expect("spawn");
    assert!(!output.status.success());
    assert_eq!(output.stderr, Vec::<u8>::new());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["status"], "err");
    assert_eq!(value["error"]["bin"], "verctl");
}

/// Agent shells force colour through `CLICOLOR_FORCE`, `FORCE_COLOR`, and
/// `COLORTERM`. JSON output, a JSON error, and a parse error under
/// `--format json` stay plain whatever those variables hold, so `jq` never
/// sees an escape byte.
#[test]
fn json_output_holds_no_escape_byte_under_forced_colour() {
    let root = TempDir::new().expect("tempdir");
    common::write_demo_package(root.path());
    let good = root.path().join("good");
    let bad = root.path().join("bad");
    fs::create_dir_all(&good).expect("dir");
    fs::create_dir_all(&bad).expect("dir");
    fs::write(bad.join("bad.md"), "nope").expect("write");
    let good = good.to_str().expect("UTF-8 path");
    let bad = bad.to_str().expect("UTF-8 path");
    let cases: [&[&str]; 5] = [
        &["status", "--format", "json", "-d", good],
        &["check", "--format", "json", "-d", bad],
        &["version", "--bogus", "--format", "json"],
        &["--format", "json", "check", "nosuchcheck"],
        &["--format=json", "nosuchverb"],
    ];
    for args in cases {
        let output = common::verctl()
            .current_dir(root.path())
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .env("FORCE_COLOR", "3")
            .env("COLORTERM", "truecolor")
            .env_remove("NO_COLOR")
            .output()
            .expect("spawn");
        assert!(
            !output.stdout.contains(&0x1b) && !output.stderr.contains(&0x1b),
            "{args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .unwrap_or_else(|error| panic!("{args:?}: stdout is not JSON: {error}"));
    }
}

#[test]
fn instructions_preserve_the_installed_markdown() {
    let output = common::verctl()
        .arg("instructions")
        .output()
        .expect("spawn");
    assert!(output.status.success());
    assert_eq!(output.stderr, Vec::<u8>::new());
    let expected = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/instructions.md"))
        .expect("instructions");
    assert_eq!(output.stdout, expected);
}

#[test]
fn check_fails_closed_on_bad_fragment() {
    let root = TempDir::new().expect("tempdir");
    let dir = root.path().join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("bad.md"),
        indoc! {"
            nope
        "},
    )
    .expect("write");
    let output = common::verctl()
        .args(["check", "-d"])
        .arg(&dir)
        .output()
        .expect("spawn");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("must start with ---")
            || String::from_utf8_lossy(&output.stderr).contains("changeset")
    );
}

#[test]
fn prepare_cli_writes_cargo() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cargo = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(cargo.contains("version = \"1.0.1\""), "{cargo}");
}

#[test]
fn prepare_dry_run_writes_nothing() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "-n", "--color", "never"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("demo"), "{stdout}");
    assert!(stdout.contains("1.0.0"), "{stdout}");
    assert!(stdout.contains("1.0.1"), "{stdout}");
    assert!(stdout.contains("patch"), "{stdout}");
    assert!(stdout.contains("## demo 1.0.1"), "{stdout}");
    assert!(stdout.contains("Patch"), "{stdout}");
    assert!(stdout.contains("dry-run"), "{stdout}");
    assert!(stdout.contains("nothing written"), "{stdout}");
    assert!(!stdout.contains("consume"), "{stdout}");
    assert!(stdout.contains('│') || stdout.contains('|'), "{stdout}");
    let cargo = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(cargo.contains("version = \"1.0.0\""), "{cargo}");
    assert!(root.path().join(".changeset/bump.md").exists());
    assert!(!root.path().join("CHANGELOG.md").exists());
}

#[test]
fn prepare_pr_preview_needs_no_auth_and_writes_nothing() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["version", "--pr", "--preview", "--color", "never"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bump.md"), "{stdout}");
    assert!(stdout.contains("consume"), "{stdout}");
    assert!(stdout.contains("verctl/release/"), "{stdout}");
    let cargo = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(cargo.contains("version = \"1.0.0\""), "{cargo}");
    assert!(root.path().join(".changeset/bump.md").exists());
}

#[test]
fn prepare_no_pr_still_writes() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "--no-pr"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cargo = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(cargo.contains("version = \"1.0.1\""), "{cargo}");
}

#[test]
fn prepare_pr_noops_without_fragments() {
    let root = TempDir::new().expect("tmp");
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .expect("cargo");
    let output = common::verctl()
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["version", "--pr", "--color", "never"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no-op"), "{stdout}");
    assert!(stdout.contains("no version-changing fragments"), "{stdout}");
}

#[test]
fn prepare_pr_fails_closed_without_github_auth() {
    let root = demo_root();
    let output = common::verctl()
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .current_dir(root.path())
        .args(["version", "--pr"])
        .output()
        .expect("spawn");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
}

#[test]
fn help_has_short_and_long() {
    for args in [["-h"].as_slice(), ["--help"].as_slice()] {
        let output = common::verctl().args(args).output().expect("spawn");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"), "{stdout}");
        assert!(stdout.contains("-h"), "{stdout}");
        assert!(stdout.contains("--help"), "{stdout}");
        assert!(stdout.contains("-V"), "{stdout}");
        assert!(stdout.contains("--version"), "{stdout}");
    }
    let output = common::verctl()
        .args(["version", "-h"])
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("-n"), "{stdout}");
    assert!(stdout.contains("--dry-run"), "{stdout}");
    assert!(stdout.contains("--pr"), "{stdout}");
    assert!(stdout.contains("--no-pr"), "{stdout}");
}

#[test]
fn check_ok_when_dir_missing() {
    let root = TempDir::new().expect("tempdir");
    let output = common::verctl()
        .args(["check", "--color", "never", "-d"])
        .arg(root.path().join("missing"))
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fragments  ok, 0 fragment(s)"), "{stdout}");
}

/// verctl never arms a Version PR's merge: a person merges it, so the
/// removed `--auto-merge` is an unknown argument, refused before any write.
#[test]
fn prepare_refuses_auto_merge() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "--pr", "--auto-merge", "--color", "never"])
        .output()
        .expect("spawn");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("--auto-merge"), "{stderr}");
    let cargo = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(cargo.contains("version = \"1.0.0\""), "{cargo}");
    assert!(root.path().join(".changeset/bump.md").exists());
}

#[test]
fn check_refuses_what_prepare_refuses() {
    let root = TempDir::new().expect("tmp");
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.0.0"
        "#},
    )
    .expect("cargo");
    let changes = root.path().join(".changeset");
    fs::create_dir_all(&changes).expect("dir");
    let check = || {
        common::verctl()
            .current_dir(root.path())
            .args(["check", "--color", "never"])
            .output()
            .expect("spawn")
    };
    fs::write(
        changes.join("first.md"),
        indoc! {"
            ---
            demo: minor
            ---

            First.
        "},
    )
    .expect("frag");
    let refused = check();
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("first_minor"), "{stderr}");
    fs::write(
        changes.join("first.md"),
        indoc! {"
            ---
            demo: patch
            ---

            First.
        "},
    )
    .expect("frag");
    let passed = check();
    assert!(
        passed.status.success(),
        "{}",
        String::from_utf8_lossy(&passed.stderr)
    );
}

/// Every long option reads `VERCTL_<LONG>`: the environment alone selects
/// JSON, and a flag on the command line wins over its variable.
#[test]
fn verctl_format_selects_json_and_the_flag_wins() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["status"])
        .env("VERCTL_FORMAT", "json")
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .unwrap_or_else(|error| panic!("stdout is not JSON: {error}"));
    let output = common::verctl()
        .current_dir(root.path())
        .args(["status", "--format", "pretty"])
        .env("VERCTL_FORMAT", "json")
        .output()
        .expect("spawn");
    assert!(output.status.success());
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_err());
}

/// A composite action sets `VERCTL_CONFIG` to `""` for an input it was not
/// given; verctl reads that as unset and uses `.ctl/ver.yaml`.
#[test]
fn an_empty_verctl_config_means_the_default() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "--dry-run", "--format", "json"])
        .env("VERCTL_CONFIG", "")
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// `check` plans without writing, so a package whose `VERSION` file does not
/// exist yet passes and the file stays missing until `version` writes it.
#[test]
fn check_leaves_a_missing_version_file_missing() {
    let root = TempDir::new().expect("tmp");
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: setup
                path: actions/setup/VERSION
        "},
    );
    fs::create_dir_all(root.path().join("actions/setup")).expect("dir");
    let changes = root.path().join(".changeset");
    fs::create_dir_all(&changes).expect("dir");
    fs::write(
        changes.join("first.md"),
        indoc! {"
            ---
            setup: patch
            ---

            First.
        "},
    )
    .expect("frag");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["check", "--color", "never"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.path().join("actions/setup/VERSION").exists());
}

/// The verbs 0.8.0 renamed are unknown commands: each exits 2 before it
/// reads any config, with clap's unrecognized subcommand error.
#[test]
fn a_renamed_verb_is_an_unknown_command() {
    let root = TempDir::new().expect("tempdir");
    for verb in ["prepare", "pin", "publish", "assets"] {
        let output = common::verctl()
            .current_dir(root.path())
            .args([verb, "--dry-run", "--color", "never"])
            .output()
            .expect("spawn");
        assert_eq!(output.status.code(), Some(2), "{verb}");
        assert_eq!(output.stdout, Vec::<u8>::new(), "{verb}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("unrecognized subcommand '{verb}'")),
            "{stderr}"
        );
    }
}

/// `docs` is an unknown command: verctl waits for no docs.rs build, so the
/// verb is gone with no hidden one left behind.
#[test]
fn docs_is_an_unknown_command() {
    let root = TempDir::new().expect("tempdir");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["docs", "--color", "never"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, Vec::<u8>::new());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized subcommand 'docs'"),
        "{stderr}"
    );
}

/// `release --plan` is an unknown flag, and under `--format json` the
/// failure is the JSON error on stdout.
#[test]
fn release_plan_is_an_unknown_flag_in_json() {
    let root = TempDir::new().expect("tempdir");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["--format", "json", "release", "--plan"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, Vec::<u8>::new());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["status"], "err");
    let message = value["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("unexpected argument '--plan'"),
        "{message}"
    );
}

/// `check --versions` is an unknown flag: `check versions` replaced it.
#[test]
fn check_versions_flag_is_an_unknown_argument() {
    let root = TempDir::new().expect("tempdir");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["check", "--versions", "--color", "never"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, Vec::<u8>::new());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--versions'"),
        "{stderr}"
    );
}

/// An unknown check exits 2 and lists the known ones.
#[test]
fn an_unknown_check_exits_2_listing_the_checks() {
    let root = TempDir::new().expect("tempdir");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["check", "release", "--color", "never"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[possible values: fragments, served, versions, registry, trust]"),
        "{stderr}"
    );
}

#[test]
fn help_leaves_out_the_removed_verbs() {
    let output = common::verctl()
        .args(["--help", "--color", "never"])
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("version"), "{stdout}");
    assert!(stdout.contains("render"), "{stdout}");
    for removed in ["prepare", "pin "] {
        assert!(
            !stdout
                .lines()
                .any(|line| line.trim_start().starts_with(removed)),
            "{stdout}"
        );
    }
}

/// The version report lists follow-up commands under `after`; 0.7 called
/// the field `next`.
#[test]
fn version_json_lists_follow_up_commands_under_after() {
    let root = demo_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "--dry-run", "--format", "json"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert!(value["after"].is_array(), "{value}");
    assert!(value.get("next").is_none(), "{value}");
}

/// A config that only vendors declares no packages, which is valid. `status`
/// plans no package assets for it, and with a token and `--github-output`
/// writes only the release outputs, which list nothing.
#[test]
fn a_config_with_no_packages_has_no_package_assets_to_plan() {
    let root = TempDir::new().expect("tempdir");
    common::write_config(
        root.path(),
        indoc! {"
            runners:
              linux:
                labels: [ubuntu-26.04]
            vendored:
              libexample:
                version: 1.4.0
                dir: out/{id}
                files: [LICENSE]
                targets:
                  linux-x64:
                    runner: linux
        "},
    );
    common::init(root.path());
    common::commit_all(root.path(), "chore: vendor");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["status", "--format", "json"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert!(value["assets"].is_null(), "{value}");

    let out = root.path().join("github-output");
    let planned = common::verctl()
        .current_dir(root.path())
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_API_URL", "http://127.0.0.1:9")
        .args(["status", "--github-output", out.to_str().expect("utf-8")])
        .output()
        .expect("spawn");
    assert!(planned.status.success(), "{planned:?}");
    assert_eq!(
        fs::read_to_string(&out).expect("output"),
        indoc! {r#"
            commits=[]
            pending=false
            assets={"include":[]}
            declared=
        "#}
    );
}

/// `package`'s manifest at `version`.
fn manifest(package: &str, version: &str) -> String {
    formatdoc! {r#"
        [package]
        name = "{package}"
        version = "{version}"
    "#}
}

/// A GitHub that holds no tag and no Release, so every version a plan holds is
/// unpublished. Each tag in `never_asked` fails the test when the plan looks it
/// up.
async fn github_without_releases(never_asked: &[&str]) -> MockServer {
    let not_found = || {
        ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "message": "Not Found",
            "documentation_url": "https://docs.github.com"
        }))
    };
    let server = MockServer::start().await;
    for tag in never_asked {
        Mock::given(method("GET"))
            .and(path(format!("/repos/acme/demo/git/ref/tags/{tag}")))
            .respond_with(not_found())
            .expect(0)
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path_regex(r"^/repos/acme/demo/git/ref/tags/.+$"))
        .respond_with(not_found())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/repos/acme/demo/releases/tags/.+$"))
        .respond_with(not_found())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    server
}

/// Put `origin` on GitHub, with `origin/main` at HEAD.
fn on_github(root: &Path) {
    common::git(
        root,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/demo.git",
        ],
    );
    let head = common::git(root, &["rev-parse", "HEAD"]);
    common::git(root, &["update-ref", "refs/remotes/origin/main", &head]);
}

/// Run `status --github-output` and return what it wrote and its JSON report.
fn plan_at_head(root: &Path, server: &MockServer) -> (String, serde_json::Value) {
    let out = root.join("github-output");
    let output = common::verctl()
        .current_dir(root)
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_API_URL", server.uri())
        .args([
            "status",
            "--github-output",
            out.to_str().expect("utf-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        fs::read_to_string(&out).expect("output"),
        serde_json::from_slice(&output.stdout).expect("JSON"),
    )
}

fn row(name: &str, version: &str, commit: &str) -> serde_json::Value {
    serde_json::json!({ "name": name, "version": version, "commit": commit })
}

/// Two packages whose versions different commits set, neither published, are
/// one plan at HEAD: the commit `A` set `core` and the commit `C` set `app`
/// both release from `C`.
#[tokio::test]
async fn packages_bumped_at_different_commits_are_planned_at_head() {
    let root = TempDir::new().expect("tempdir");
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: core
                path: core/Cargo.toml
                publisher: none
              - name: app
                path: app/Cargo.toml
                publisher: none
            tags:
              template: "{name}@{version}"
        "#},
    );
    for (package, version) in [("core", "0.3.0"), ("app", "0.2.0")] {
        fs::create_dir_all(root.path().join(package)).expect("dir");
        fs::write(
            root.path().join(package).join("Cargo.toml"),
            manifest(package, version),
        )
        .expect("manifest");
    }
    common::init(root.path());
    common::commit_all(root.path(), "chore: start");
    fs::write(
        root.path().join("core/Cargo.toml"),
        manifest("core", "0.3.1"),
    )
    .expect("manifest");
    common::commit_all(root.path(), "chore(release): core 0.3.1");
    fs::write(root.path().join("app/Cargo.toml"), manifest("app", "0.2.1")).expect("manifest");
    let head = common::commit_all(root.path(), "chore(release): app 0.2.1");
    on_github(root.path());
    let server = github_without_releases(&[]).await;
    let (written, report) = plan_at_head(root.path(), &server);
    assert!(
        written.contains(&format!(r#"commits=["{head}"]"#)),
        "{written}"
    );
    assert!(written.contains("pending=true"), "{written}");
    assert_eq!(
        report["unreleased"],
        serde_json::json!([row("core", "0.3.1", &head), row("app", "0.2.1", &head)])
    );
}

/// A version HEAD does not hold stays unreleased.
/// The commit `A` set `core` to 0.3.1, and the commit `C` set it to 0.4.0.
/// The plan holds 0.4.0 only, and GitHub is never asked about 0.3.1.
#[tokio::test]
async fn a_version_head_no_longer_holds_is_not_planned() {
    let root = TempDir::new().expect("tempdir");
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: core
                path: Cargo.toml
                publisher: none
            tags:
              template: "{name}@{version}"
        "#},
    );
    fs::write(root.path().join("Cargo.toml"), manifest("core", "0.3.0")).expect("manifest");
    common::init(root.path());
    common::commit_all(root.path(), "chore: start");
    fs::write(root.path().join("Cargo.toml"), manifest("core", "0.3.1")).expect("manifest");
    common::commit_all(root.path(), "chore(release): core 0.3.1");
    fs::write(root.path().join("Cargo.toml"), manifest("core", "0.4.0")).expect("manifest");
    let head = common::commit_all(root.path(), "chore(release): core 0.4.0");
    on_github(root.path());
    let server = github_without_releases(&["core@0.3.1"]).await;
    let (written, report) = plan_at_head(root.path(), &server);
    assert!(
        written.contains(&format!(r#"commits=["{head}"]"#)),
        "{written}"
    );
    assert_eq!(
        report["unreleased"],
        serde_json::json!([row("core", "0.4.0", &head)])
    );
}

/// HEAD one commit past the commit that set the version, with the version
/// unpublished: the plan holds the version at HEAD.
#[tokio::test]
async fn head_past_the_version_commit_plans_the_version_at_head() {
    let root = TempDir::new().expect("tempdir");
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
        "},
    );
    fs::write(root.path().join("Cargo.toml"), manifest("demo", "0.4.5")).expect("manifest");
    common::init(root.path());
    common::commit_all(root.path(), "chore: start");
    fs::write(root.path().join("Cargo.toml"), manifest("demo", "0.4.6")).expect("manifest");
    common::commit_all(root.path(), "chore(release): demo 0.4.6");
    fs::write(root.path().join("README"), "later\n").expect("readme");
    let head = common::commit_all(root.path(), "docs: later");
    on_github(root.path());
    let server = github_without_releases(&[]).await;
    let (written, report) = plan_at_head(root.path(), &server);
    assert!(
        written.contains(&format!(r#"commits=["{head}"]"#)),
        "{written}"
    );
    assert!(written.contains("pending=true"), "{written}");
    assert_eq!(
        report["unreleased"],
        serde_json::json!([row("demo", "0.4.6", &head)])
    );
}

/// A release tag at an older commit at the same version does not move the plan.
/// The commit `A` set `demo` to 0.4.6 and carries the tag, and no Release
/// exists.
/// HEAD is the commit `B` on top, so the plan holds the version at `B`.
#[tokio::test]
async fn a_tag_at_an_older_commit_still_plans_head() {
    let root = TempDir::new().expect("tempdir");
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
        "},
    );
    fs::write(root.path().join("Cargo.toml"), manifest("demo", "0.4.5")).expect("manifest");
    common::init(root.path());
    common::commit_all(root.path(), "chore: start");
    fs::write(root.path().join("Cargo.toml"), manifest("demo", "0.4.6")).expect("manifest");
    let set = common::commit_all(root.path(), "chore(release): demo 0.4.6");
    fs::write(root.path().join("README"), "later\n").expect("readme");
    let head = common::commit_all(root.path(), "docs: later");
    on_github(root.path());
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/git/ref/tags/v0.4.6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/tags/v0.4.6",
            "node_id": "R",
            "url": "https://api.github.com/repos/acme/demo/git/refs/tags/v0.4.6",
            "object": {
                "type": "commit",
                "sha": set,
                "url": format!("https://api.github.com/repos/acme/demo/git/commits/{set}")
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/tags/v0.4.6"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "message": "Not Found",
            "documentation_url": "https://docs.github.com"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    let (written, report) = plan_at_head(root.path(), &server);
    assert!(
        written.contains(&format!(r#"commits=["{head}"]"#)),
        "{written}"
    );
    assert!(written.contains("pending=true"), "{written}");
    assert_eq!(
        report["unreleased"],
        serde_json::json!([row("demo", "0.4.6", &head)])
    );
}

/// A target added after a release never reopens it: the commit `A` set
/// `demo` to 0.4.6 with no asset target, and its tag and published Release
/// exist there. HEAD `B` declares `linux-x64`, so the release is checked
/// against the config at `A`, which declares none, and nothing is pending.
#[tokio::test]
async fn a_target_added_after_the_release_does_not_reopen_it() {
    let root = TempDir::new().expect("tempdir");
    let config = indoc! {"
        packages:
          - name: demo
            path: Cargo.toml
            publisher: none
    "};
    common::write_config(root.path(), config);
    fs::write(root.path().join("Cargo.toml"), manifest("demo", "0.4.6")).expect("manifest");
    common::init(root.path());
    let set = common::commit_all(root.path(), "chore(release): demo 0.4.6");
    common::write_config(
        root.path(),
        &formatdoc! {r#"
            {config}assets:
              build: ["true"]
              binary: out/{{bin}}
              linux-x64: {{}}
        "#},
    );
    common::commit_all(root.path(), "ci: build a linux-x64 tarball");
    on_github(root.path());
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/git/ref/tags/v0.4.6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/tags/v0.4.6",
            "node_id": "R",
            "url": "https://api.github.com/repos/acme/demo/git/refs/tags/v0.4.6",
            "object": {
                "type": "commit",
                "sha": set,
                "url": format!("https://api.github.com/repos/acme/demo/git/commits/{set}")
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/tags/v0.4.6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "html_url": "https://github.com/acme/demo/releases/tag/v0.4.6",
            "tag_name": "v0.4.6",
            "draft": false,
            "immutable": false,
            "assets": []
        })))
        .mount(&server)
        .await;
    let (written, report) = plan_at_head(root.path(), &server);
    assert!(written.contains("commits=[]"), "{written}");
    assert!(written.contains("pending=false"), "{written}");
    assert_eq!(report["unreleased"], serde_json::json!([]));
}
