mod common;

use common::demo_root;
use indoc::indoc;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn status_lists_mixed_packages() {
    let root = TempDir::new().expect("tempdir");
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .args(["status", "-d"])
        .arg(&dir)
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pending  1"));
    assert!(stdout.contains("forkctl"));
    assert!(stdout.contains("@scope/pkg"));
    assert!(stdout.contains("max     minor"));
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["prepare"])
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["prepare", "-n"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bump    demo  1.0.0 -> 1.0.1"), "{stdout}");
    assert!(stdout.contains("log     ## demo 1.0.1"), "{stdout}");
    assert!(stdout.contains("Patch"), "{stdout}");
    assert!(stdout.contains("dry-run (no files written)"), "{stdout}");
    assert!(!stdout.contains("consume "), "{stdout}");
    let cargo = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(cargo.contains("version = \"1.0.0\""), "{cargo}");
    assert!(root.path().join(".changeset/bump.md").exists());
    assert!(!root.path().join("CHANGELOG.md").exists());
}

#[test]
fn prepare_pr_preview_needs_no_auth_and_writes_nothing() {
    let root = demo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["prepare", "--pr", "--preview"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("consume bump.md"), "{stdout}");
    assert!(
        stdout.contains("pr      open or update version-packages"),
        "{stdout}"
    );
    let cargo = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(cargo.contains("version = \"1.0.0\""), "{cargo}");
    assert!(root.path().join(".changeset/bump.md").exists());
}

#[test]
fn prepare_no_pr_still_writes() {
    let root = demo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["prepare", "--no-pr"])
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
fn prepare_pr_fails_closed_without_github_auth() {
    let root = demo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .current_dir(root.path())
        .args(["prepare", "--pr"])
        .output()
        .expect("spawn");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("GITHUB_TOKEN"), "{stderr}");
}

#[test]
fn help_has_short_and_long() {
    for args in [["-h"].as_slice(), ["--help"].as_slice()] {
        let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
            .args(args)
            .output()
            .expect("spawn");
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .args(["prepare", "-h"])
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .args(["check", "-d"])
        .arg(root.path().join("missing"))
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "ok      0 fragment(s)"
    );
}
