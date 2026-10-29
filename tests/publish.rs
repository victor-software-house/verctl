//! `publish --dry-run` prints the stock command plan and writes nothing.

mod common;

use std::fs;

use indoc::{formatdoc, indoc};
use tempfile::TempDir;

fn publish_stdout(root: &std::path::Path) -> String {
    let output = common::verctl()
        .current_dir(root)
        .args(["publish", "--dry-run", "--color", "never"])
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

/// A declared asset makes the Release a draft until it is uploaded, and
/// `--skip-assets` publishes it at once.
#[test]
fn dry_run_drafts_a_release_with_declared_files() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
            assets:
              files: [build/demo.pkg]
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
    assert!(
        publish_stdout(root.path()).contains("release  would create draft v0.0.1\n"),
        "{}",
        publish_stdout(root.path())
    );
    let output = common::verctl()
        .current_dir(root.path())
        .args(["publish", "--dry-run", "--skip-assets", "--color", "never"])
        .output()
        .unwrap_or_else(|error| panic!("spawn verctl: {error}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("release  would create v0.0.1\n"),
        "{stdout}"
    );
}

#[test]
fn dry_run_lists_many_packages() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
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
#[test]
fn dry_run_leaves_out_a_package_at_0_0_0() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
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
    assert_eq!(publish_stdout(root.path()), expected);
}

/// A dry run agrees with `publish`: a repository whose only package is at
/// `0.0.0` has nothing to release.
#[test]
fn dry_run_of_a_package_at_0_0_0_has_nothing_to_release() {
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
    let stdout = publish_stdout(root.path());
    assert!(stdout.contains("nothing to release"), "{stdout}");
}

#[test]
fn dry_run_refuses_differing_versions_without_name() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
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
        .args(["publish", "--dry-run", "--color", "never"])
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
        .args(["publish", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("changelog section"), "{stderr}");
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
    let output = common::verctl()
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["publish", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
    assert!(!stderr.contains("changelog section"), "{stderr}");
}

/// A crate that adopts the release workflow at `0.0.0` has nothing to
/// release. The GitHub API address is closed, so any lookup for the package
/// would fail the plan.
#[test]
fn a_package_at_0_0_0_plans_nothing() {
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
        .env("GITHUB_OUTPUT", &github_output)
        .args(["publish", "--plan", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nothing to release"), "{stdout}");
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
    let output = common::verctl()
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["publish", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
    assert!(!stderr.contains("changelog section"), "{stderr}");
}
