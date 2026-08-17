//! `publish --dry-run` prints the crates on HEAD and writes nothing.
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
    assert!(stdout.contains("demo@0.0.1"), "{stdout}");
    assert!(stdout.contains("dry-run"), "{stdout}");
}

#[test]
fn empty_prepare_pr_is_noop() {
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
    fs::create_dir_all(root.path().join(".changeset")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["prepare", "--pr"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.contains("no-op"), "{stdout}");
}
