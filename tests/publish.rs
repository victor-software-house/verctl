//! `publish --dry-run` prints the stock command plan and writes nothing.
#![allow(missing_docs)]

use indoc::indoc;
use std::fs;
use tempfile::TempDir;

#[test]
fn dry_run_lists_cargo_crate() {
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
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["publish", "--dry-run"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(
        stdout,
        indoc! {"
            crate   demo@0.0.1 (cargo)
            release would create v0.0.1
            dry-run (nothing published)
        "}
    );
}

#[test]
fn dry_run_lists_bun_package() {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "@org/pkg"
            path = "package.json"
            registry = "github"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "0.0.1" }
        "#},
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["publish", "--dry-run"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(
        stdout,
        indoc! {"
            package @org/pkg@0.0.1 (bun github)
            release would create v0.0.1
            dry-run (nothing published)
        "}
    );
}
