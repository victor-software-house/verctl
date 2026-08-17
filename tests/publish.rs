//! `publish --dry-run` prints the stock command plan and writes nothing.
#![allow(missing_docs)]

use ctl_core::{ColorMode, grid, kv};
use indoc::indoc;
use std::fs;
use tempfile::TempDir;

#[allow(clippy::unwrap_used)]
fn publish_stdout(root: &std::path::Path) -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root)
        .args(["publish", "--dry-run", "--color", "never"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn dry_run_lists_one_cargo_crate() {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.0.1"
        "#},
    )
    .unwrap();
    let mut expected = grid(
        ColorMode::Never,
        &["name", "version", "via"],
        [vec!["demo".into(), "0.0.1".into(), "cargo".into()]],
    );
    expected.push('\n');
    expected.push_str(&kv(
        ColorMode::Never,
        [
            ("release", "would create v0.0.1"),
            ("dry-run", "nothing published"),
        ],
    ));
    assert_eq!(publish_stdout(root.path()), expected);
}

#[test]
fn dry_run_lists_many_packages() {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
            [[packages]]
            name = "@org/pkg"
            path = "package.json"
            registry = "github"
        "#},
    )
    .unwrap();
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
    let mut expected = grid(
        ColorMode::Never,
        &["name", "version", "via"],
        [
            vec!["demo".into(), "0.0.1".into(), "cargo".into()],
            vec!["@org/pkg".into(), "0.0.2".into(), "bun github".into()],
        ],
    );
    expected.push('\n');
    expected.push_str(&kv(
        ColorMode::Never,
        [
            ("release", "would create v0.0.1"),
            ("dry-run", "nothing published"),
        ],
    ));
    assert_eq!(publish_stdout(root.path()), expected);
}
