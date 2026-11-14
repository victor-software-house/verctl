//! `publish --dry-run` prints the stock plan and writes nothing.
//!
//! Each `dry_run_*` fixture sets `release_checks: []`, because it runs in a
//! directory with no git repository and no network, where the release proof
//! a dry run runs could not resolve the default branch or reach crates.io.

mod common;

use std::fs;
use std::path::Path;

use indoc::{formatdoc, indoc};
use tempfile::TempDir;
use verctl::registry::CRATES_IO_INDEX;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn publish_stdout(root: &Path) -> String {
    publish_stdout_at(root, CRATES_IO_INDEX)
}

/// The dry run's stdout, reading the crates.io index at `index`, which a
/// package at `0.0.0` makes the run read.
fn publish_stdout_at(root: &Path, index: &str) -> String {
    let output = common::verctl()
        .current_dir(root)
        .args(["release", "--dry-run", "--color", "never"])
        .args(["--crates-io-index", index])
        .output()
        .unwrap_or_else(|error| panic!("spawn verctl: {error}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn dry_run_lists_one_cargo_crate() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
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
            version = "0.0.1"
        "#},
    )
    .unwrap();
    let expected = formatdoc! {"
        ┌──────┬─────────┬───────┐
        │ name ┆ version ┆ via   │
        ╞══════╪═════════╪═══════╡
        │ demo ┆ 0.0.1   ┆ cargo │
        └──────┴─────────┴───────┘

        release  would create v0.0.1
        dry-run  nothing published
    "};
    assert_eq!(publish_stdout(root.path()), expected);
}

/// A package at a virtual workspace manifest publishes each member that
/// publishes, dependencies first, and leaves out `publish = false`.
#[test]
fn dry_run_lists_workspace_members_in_publish_order() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    let write = |path: &str, body: &str| {
        let path = root.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    };
    write(
        "Cargo.toml",
        indoc! {r#"
            [workspace]
            resolver = "3"
            members = ["app", "core", "xtask"]

            [workspace.package]
            version = "0.0.2"
            edition = "2024"
        "#},
    );
    write(
        "app/Cargo.toml",
        indoc! {r#"
            [package]
            name = "demo-app"
            version.workspace = true
            edition.workspace = true

            [dependencies]
            demo-core = { version = "=0.0.2", path = "../core" }
        "#},
    );
    write("app/src/main.rs", "fn main() {}");
    write(
        "core/Cargo.toml",
        indoc! {r#"
            [package]
            name = "demo-core"
            version.workspace = true
            edition.workspace = true
        "#},
    );
    write("core/src/lib.rs", "");
    write(
        "xtask/Cargo.toml",
        indoc! {r#"
            [package]
            name = "xtask"
            version = "0.1.0"
            edition.workspace = true
            publish = false
        "#},
    );
    write("xtask/src/main.rs", "fn main() {}");
    let expected = formatdoc! {"
        ┌───────────┬─────────┬───────┐
        │ name      ┆ version ┆ via   │
        ╞═══════════╪═════════╪═══════╡
        │ demo-core ┆ 0.0.2   ┆ cargo │
        │ demo-app  ┆ 0.0.2   ┆ cargo │
        └───────────┴─────────┴───────┘

        release  would create v0.0.2
        dry-run  nothing published
    "};
    assert_eq!(publish_stdout(root.path()), expected);
}

/// A private binary with `publish = false` releases on GitHub only.
#[test]
fn dry_run_lists_a_release_only_package() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
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
            version = "0.0.1"
            publish = false
        "#},
    )
    .unwrap();
    let expected = formatdoc! {"
        ┌──────┬─────────┬──────────────┐
        │ name ┆ version ┆ via          │
        ╞══════╪═════════╪══════════════╡
        │ demo ┆ 0.0.1   ┆ release only │
        └──────┴─────────┴──────────────┘

        release  would create v0.0.1
        dry-run  nothing published
    "};
    assert_eq!(publish_stdout(root.path()), expected);
}

/// A cargo option that differs from its default shows beside the publisher.
#[test]
fn dry_run_lists_a_crate_that_skips_the_verify_build() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
            packages:
              - name: demo
                path: Cargo.toml
                publish:
                  verify: false
        "},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.0.1"
        "#},
    )
    .unwrap();
    let expected = formatdoc! {"
        ┌──────┬─────────┬──────────────────┐
        │ name ┆ version ┆ via              │
        ╞══════╪═════════╪══════════════════╡
        │ demo ┆ 0.0.1   ┆ cargo, no verify │
        └──────┴─────────┴──────────────────┘

        release  would create v0.0.1
        dry-run  nothing published
    "};
    assert_eq!(publish_stdout(root.path()), expected);
}

/// A manifest that opts out of publishing releases on GitHub only, with no
/// `publisher: none` in `.ctl/ver.yaml`.
#[test]
fn dry_run_releases_a_private_crate_on_github_only() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
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
            version = "0.0.1"
            publish = false
        "#},
    )
    .unwrap();
    let expected = formatdoc! {"
        ┌──────┬─────────┬──────────────┐
        │ name ┆ version ┆ via          │
        ╞══════╪═════════╪══════════════╡
        │ demo ┆ 0.0.1   ┆ release only │
        └──────┴─────────┴──────────────┘

        release  would create v0.0.1
        dry-run  nothing published
    "};
    assert_eq!(publish_stdout(root.path()), expected);
}

#[test]
fn dry_run_lists_many_packages() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
            release_checks: []
            packages:
              - name: demo
                path: Cargo.toml
              - name: "@org/pkg"
                path: package.json
                registry: github

            tags:
              template: "{name}@{version}"
        "#},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.0.1"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "0.0.2" }
        "#},
    )
    .unwrap();
    let expected = formatdoc! {"
        ┌──────────┬─────────┬────────────┐
        │ name     ┆ version ┆ via        │
        ╞══════════╪═════════╪════════════╡
        │ demo     ┆ 0.0.1   ┆ cargo      │
        │ @org/pkg ┆ 0.0.2   ┆ bun github │
        └──────────┴─────────┴────────────┘

        release  would create demo@0.0.1
        release  would create @org/pkg@0.0.2
        dry-run  nothing published
    "};
    assert_eq!(publish_stdout(root.path()), expected);
}

/// A new package at `0.0.0` beside a released one is left out, so the shared
/// `v{version}` tag still names one version.
#[tokio::test]
async fn dry_run_leaves_out_a_package_at_0_0_0() {
    let index = MockServer::start().await;
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
            packages:
              - name: demo
                path: Cargo.toml
              - name: extra
                path: extra/Cargo.toml
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
    .unwrap();
    fs::create_dir_all(root.path().join("extra")).unwrap();
    fs::write(
        root.path().join("extra/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "extra"
            version = "0.0.0"
        "#},
    )
    .unwrap();
    let expected = formatdoc! {"
        ┌──────┬─────────┬───────┐
        │ name ┆ version ┆ via   │
        ╞══════╪═════════╪═══════╡
        │ demo ┆ 1.0.0   ┆ cargo │
        └──────┴─────────┴───────┘

        release  would create v1.0.0
        dry-run  nothing published
    "};
    assert_eq!(publish_stdout_at(root.path(), &index.uri()), expected);
}

/// A dry run matches `publish`: a repository whose only package is at
/// `0.0.0`, and that crates.io lacks, has nothing to release. The index it
/// reads is a recorded one, with no crate.
#[tokio::test]
async fn dry_run_of_a_package_at_0_0_0_has_nothing_to_release() {
    let index = MockServer::start().await;
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
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
    .unwrap();
    let stdout = publish_stdout_at(root.path(), &index.uri());
    assert!(stdout.contains("nothing to release"), "{stdout}");
}

/// A `VERSION` package before its first release has no file yet. The missing
/// file reads as `0.0.0`, so there is nothing to release and no missing-file
/// error.
#[test]
fn dry_run_of_a_missing_version_file_has_nothing_to_release() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
            packages:
              - name: templates
                path: VERSION
        "},
    );
    let stdout = publish_stdout(root.path());
    assert!(stdout.contains("nothing to release"), "{stdout}");
}

/// When `publish` cannot read a version file, it fails and names the package,
/// as `version` does.
#[test]
fn dry_run_names_the_package_whose_version_it_cannot_read() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            release_checks: []
            packages:
              - name: templates
                path: VERSION
        "},
    );
    fs::write(
        root.path().join("VERSION"),
        indoc! {"
            1.2.3
            notes
        "},
    )
    .unwrap();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["release", "--dry-run", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("package templates"), "{stderr}");
    assert!(stderr.contains("not one version"), "{stderr}");
}

#[test]
fn dry_run_refuses_differing_versions_without_name() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
            release_checks: []
            packages:
              - name: demo
                path: Cargo.toml
              - name: "@org/pkg"
                path: package.json
                registry: github
        "#},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.0.1"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "0.0.2" }
        "#},
    )
    .unwrap();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["release", "--dry-run", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("{name}"), "{stderr}");
    assert!(stderr.contains("demo@0.0.1"), "{stderr}");
    assert!(stderr.contains("@org/pkg@0.0.2"), "{stderr}");
}

#[test]
fn publish_without_changelog_section_fails() {
    let root = TempDir::new().unwrap();
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
            version = "0.0.1"
        "#},
    )
    .unwrap();
    let output = common::verctl()
        .current_dir(root.path())
        .args(["release", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("release needs a changelog section"),
        "{stderr}"
    );
    assert!(stderr.contains("demo@0.0.1"), "{stderr}");
}

#[test]
fn publish_with_matching_changelog_asks_for_token() {
    let root = TempDir::new().unwrap();
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
            version = "0.0.1"
        "#},
    )
    .unwrap();
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
    let output = common::verctl()
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["release", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("GITHUB_TOKEN or GH_TOKEN"), "{stderr}");
    assert!(!stderr.contains("version --pr"), "{stderr}");
    assert!(!stderr.contains("changelog section"), "{stderr}");
}

/// Both token variables set fail the run before it reads any file, and the
/// error shows neither token. The directory holds no `.ctl/ver.yaml`, so a
/// run that got as far as the config would fail on that instead.
#[test]
fn both_registry_tokens_fail_before_any_work() {
    let root = TempDir::new().unwrap();
    let output = common::verctl()
        .current_dir(root.path())
        .env("VERCTL_REGISTRY_TOKEN", "from-secret")
        .env("VERCTL_TRUSTED_PUBLISHING_TOKEN", "from-trusted")
        .args(["release", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("VERCTL_REGISTRY_TOKEN and VERCTL_TRUSTED_PUBLISHING_TOKEN are both set"),
        "{stderr}"
    );
    assert!(!stderr.contains("from-"), "{stderr}");
}

/// A token variable name a shell could not export is a usage error, from the
/// flag or from its variable.
#[test]
fn a_registry_token_variable_that_is_no_name_is_a_usage_error() {
    let root = TempDir::new().unwrap();
    for (args, env) in [
        (
            ["release", "--registry-token-env", "1TOKEN"].as_slice(),
            None,
        ),
        (
            ["release"].as_slice(),
            Some(("VERCTL_REGISTRY_TOKEN_ENV", "CARGO-TOKEN")),
        ),
    ] {
        let mut command = common::verctl();
        command
            .current_dir(root.path())
            .env("VERCTL_REGISTRY_TOKEN", "t0ken")
            .args(args);
        if let Some((key, value)) = env {
            command.env(key, value);
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(
            stderr.contains("is not a variable name"),
            "{args:?}: {stderr}"
        );
        assert!(!stderr.contains("t0ken"), "{args:?}: {stderr}");
    }
}

/// A crate that adopts the release workflow at `0.0.0` has nothing to
/// release. The GitHub API address is closed, so any lookup there for the
/// package would fail the plan. Only the crates.io index is read, from a
/// recorded one with no crate.
#[tokio::test]
async fn a_package_at_0_0_0_plans_nothing() {
    let index = MockServer::start().await;
    let root = TempDir::new().unwrap();
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
    .unwrap();
    common::init(root.path());
    common::commit_all(root.path(), "chore: adopt");
    let github_output = root.path().join("github-output");
    let output = common::verctl()
        .current_dir(root.path())
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_API_URL", "http://127.0.0.1:9")
        .arg("status")
        .arg("--github-output")
        .arg(&github_output)
        .args(["--crates-io-index", &index.uri()])
        .args(["--format", "json"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["unreleased"], serde_json::json!([]));
    let written = fs::read_to_string(&github_output).unwrap();
    assert!(written.contains("pending=false"), "{written}");
}

/// `publish` asks no changelog section of a package at `0.0.0`, and goes on
/// to the token check.
#[test]
fn publish_asks_no_changelog_of_a_package_at_0_0_0() {
    let root = TempDir::new().unwrap();
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
    .unwrap();
    common::on_default_branch(root.path());
    let output = common::verctl()
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["release", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
    assert!(!stderr.contains("changelog section"), "{stderr}");
}

/// A crate at 0.0.1 with its changelog section, committed as the tip of
/// `main` with an `origin` whose `main` is that commit.
fn released_crate(root: &Path) {
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
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.0.1"
        "#},
    )
    .unwrap_or_else(|error| panic!("write fixture: {error}"));
    fs::write(
        root.join("CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## demo 0.0.1

            - First.
        "},
    )
    .unwrap_or_else(|error| panic!("write fixture: {error}"));
    fs::create_dir_all(root.join(".github/workflows"))
        .unwrap_or_else(|error| panic!("create .github/workflows: {error}"));
    fs::write(root.join(".github/workflows/ci.yml"), "on: push\n")
        .unwrap_or_else(|error| panic!("write fixture: {error}"));
    common::on_default_branch(root);
}

/// `publish` with no token, so a run that gets past every proof stops at the
/// token check, before any publisher, tag, or Release call.
fn publish_without_token(root: &Path, env: &[(&str, &str)]) -> String {
    let output = common::verctl()
        .current_dir(root)
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .envs(env.iter().copied())
        .args(["release", "--color", "never"])
        .output()
        .unwrap_or_else(|error| panic!("spawn verctl: {error}"));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(!output.status.success(), "{stderr}");
    stderr
}

/// Retrying a release commit is refused before any token is needed when a
/// workflow changed on `main`. The variable lets it go on.
#[test]
fn a_changed_workflow_refuses_before_any_publish() {
    let root = TempDir::new().unwrap();
    released_crate(root.path());
    let release = common::git(root.path(), &["rev-parse", "HEAD"]);
    fs::write(
        root.path().join(".github/workflows/ci.yml"),
        "on: [push, pull_request]\n",
    )
    .unwrap();
    let tip = common::commit_all(root.path(), "ci: run on pull requests");
    common::git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", &tip],
    );
    common::git(root.path(), &["checkout", "-q", "--detach", &release]);

    let stderr = publish_without_token(root.path(), &[]);
    assert!(stderr.contains(".github/workflows/"), "{stderr}");
    assert!(stderr.contains("--allow-workflow-change"), "{stderr}");
    assert!(!stderr.contains("GITHUB_TOKEN"), "{stderr}");

    let stderr = publish_without_token(root.path(), &[("VERCTL_ALLOW_WORKFLOW_CHANGE", "true")]);
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
}

/// A commit `main` does not contain refuses before any token is needed, and
/// the variable lets it go on.
#[test]
fn a_commit_off_the_default_branch_refuses_before_any_publish() {
    let root = TempDir::new().unwrap();
    released_crate(root.path());
    fs::write(root.path().join("README"), "ahead\n").unwrap();
    common::commit_all(root.path(), "docs: ahead of main");

    let stderr = publish_without_token(root.path(), &[]);
    assert!(stderr.contains("not an ancestor"), "{stderr}");
    assert!(stderr.contains("release refuses this commit"), "{stderr}");
    assert!(stderr.contains("to release anyway"), "{stderr}");
    assert!(stderr.contains("--allow-off-default-branch"), "{stderr}");
    assert!(!stderr.contains("GITHUB_TOKEN"), "{stderr}");

    let stderr = publish_without_token(root.path(), &[("VERCTL_ALLOW_OFF_DEFAULT_BRANCH", "true")]);
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
}

/// The completeness lookup inside `release` asks the registry with the token
/// `release` holds, under the variable `--registry-token-env` names.
/// The declared `query` lists the version only for that token, and GitHub holds
/// the tag and the Release.
/// A lookup that went without the token would find the release incomplete and
/// go on to publish.
#[tokio::test]
async fn the_completeness_lookup_asks_the_registry_with_the_token() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml
                query: ["sh", "-c", 'test "$REGISTRY_TOKEN" = t0ken && echo 0.0.1']
        "#},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.0.1"
        "#},
    )
    .unwrap();
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
    let head = common::git(root.path(), &["rev-parse", "HEAD"]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/git/ref/tags/v0.0.1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/tags/v0.0.1",
            "node_id": "R",
            "url": "https://api.github.com/repos/acme/demo/git/refs/tags/v0.0.1",
            "object": {
                "type": "commit",
                "sha": head,
                "url": format!("https://api.github.com/repos/acme/demo/git/commits/{head}")
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/demo/releases/tags/v0.0.1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "html_url": "https://github.com/acme/demo/releases/tag/v0.0.1",
            "tag_name": "v0.0.1",
            "draft": false,
            "assets": []
        })))
        .mount(&server)
        .await;
    let output = common::verctl()
        .current_dir(root.path())
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_TOKEN", "test")
        .env("GITHUB_API_URL", server.uri())
        .env("VERCTL_REGISTRY_TOKEN", "t0ken")
        .args([
            "release",
            "--registry-token-env",
            "REGISTRY_TOKEN",
            "--color",
            "never",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nothing to release"), "{stdout}");
}
