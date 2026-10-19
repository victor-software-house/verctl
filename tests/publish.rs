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
