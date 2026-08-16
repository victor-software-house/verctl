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
    fs::write(dir.join("bad.md"), "nope\n").expect("write");
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
