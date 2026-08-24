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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .args(["status", "--format", "json", "--color", "always", "-d"])
        .arg(&dir)
        .output()
        .expect("spawn");
    assert!(output.status.success());
    assert_eq!(output.stderr, &[] as &[u8]);
    assert!(!output.stdout.contains(&0x1b));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["pending"], 1);
    assert_eq!(value["max"], "patch");
    assert_eq!(value["fragments"][0]["packages"][0]["name"], "demo");
}

#[test]
fn quiet_suppresses_only_successful_pretty_status() {
    let root = TempDir::new().expect("tempdir");
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .args(["status", "--quiet", "-d"])
        .arg(root.path().join("missing"))
        .output()
        .expect("spawn");
    assert!(output.status.success());
    assert_eq!(output.stdout, &[] as &[u8]);
    assert_eq!(output.stderr, &[] as &[u8]);
}

#[test]
fn json_errors_use_stdout_and_one_envelope() {
    let root = TempDir::new().expect("tempdir");
    let dir = root.path().join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("bad.md"), "nope").expect("write");
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .args(["check", "--format", "json", "-d"])
        .arg(&dir)
        .output()
        .expect("spawn");
    assert!(!output.status.success());
    assert_eq!(output.stderr, &[] as &[u8]);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["status"], "err");
    assert_eq!(value["error"]["bin"], "verctl");
}

#[test]
fn instructions_preserve_the_installed_markdown() {
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .arg("instructions")
        .output()
        .expect("spawn");
    assert!(output.status.success());
    assert_eq!(output.stderr, &[] as &[u8]);
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
        .args(["prepare", "-n", "--color", "never"])
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["prepare", "--pr", "--preview", "--color", "never"])
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
    assert!(stdout.contains("version-packages"), "{stdout}");
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
    let output = Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["prepare", "--pr", "--color", "never"])
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
