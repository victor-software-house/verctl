//! The `status`, `check`, and `version` commands as run through the binary,
//! including JSON output.

mod common;

use std::fs;

use common::demo_root;
use indoc::indoc;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
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
/// commit lies past the shallow boundary, with a token set. `status` reports
/// the walk's failure as the reason and exits zero; the plan for
/// `--github-output` fails on it.
#[test]
fn a_depth_1_clone_leaves_unreleased_unchecked() {
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
            .is_some_and(|reason| reason.contains("the clone is shallow")),
        "{value}"
    );
    let pretty = status(&["--color", "never"]);
    assert!(pretty.status.success(), "{pretty:?}");
    let stdout = String::from_utf8_lossy(&pretty.stdout);
    assert!(stdout.contains("not checked ("), "{stdout}");
    assert!(stdout.contains("the clone is shallow"), "{stdout}");
    let out = work.path().join("out");
    let planned = status(&[
        "--format",
        "json",
        "--github-output",
        out.to_str().expect("utf-8"),
    ]);
    assert!(!planned.status.success(), "{planned:?}");
    assert!(
        String::from_utf8_lossy(&planned.stdout).contains("the clone is shallow"),
        "{planned:?}"
    );
}

/// `verctl status --format json` with a token, in a repository at
/// `demo@1.0.0` whose `origin` is GitHub, where GitHub answers the lookup of
/// tag `v1.0.0` with `answer`.
#[expect(
    clippy::expect_used,
    reason = "fixture setup fails the test outright; the status run is the result"
)]
async fn status_with_tag_lookup(answer: ResponseTemplate) -> std::process::Output {
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

/// A release tag GitHub reports on a commit HEAD does not hold at that
/// version is a wrong answer, not a lookup that could not run, so `status`
/// fails on it even without `--github-output`.
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

/// A 5xx, even an HTML page from a proxy, and a spent rate limit (a 429, or a
/// 403 with `x-ratelimit-remaining: 0`) are no answer, so `status` reports
/// the check as not run.
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
/// `--format json` stay plain whatever they say, so `jq` never sees an ESC.
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
        &["--format", "json", "pin", "--dry-run"],
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
    let expected =
        fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/instructions.md"))
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
    assert!(stdout.contains("ok"), "{stdout}");
    assert!(stdout.contains("0 fragment(s)"), "{stdout}");
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

/// `prepare` is now `version`. The hidden verb takes any arguments, reads no
/// config, and fails as a usage error that names the replacement.
#[test]
fn prepare_exits_2_and_names_version() {
    let root = TempDir::new().expect("tempdir");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["prepare", "--pr", "--color", "never"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, Vec::<u8>::new());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("prepare is now version: run verctl version"),
        "{stderr}"
    );
}

/// `pin` is now `render`. Under `--format json` the failure is the JSON error
/// on stdout.
#[test]
fn pin_exits_2_and_names_render_in_json() {
    let root = TempDir::new().expect("tempdir");
    let output = common::verctl()
        .current_dir(root.path())
        .args(["--format", "json", "pin", "--dry-run"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, Vec::<u8>::new());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["status"], "err");
    let message = value["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("pin is now render: run verctl render"),
        "{message}"
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
