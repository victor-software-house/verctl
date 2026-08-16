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
fn prepare_cli_writes_cargo() {
    use std::process::Command;
    let root = TempDir::new().expect("tmp");
    fs::write(
        root.path().join("verctl.toml"),
        "[[packages]]\nname = \"demo\"\npath = \"Cargo.toml\"\n",
    )
    .expect("cfg");
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    )
    .expect("cargo");
    let changes = root.path().join(".changeset");
    fs::create_dir_all(&changes).expect("dir");
    fs::write(changes.join("bump.md"), "---\ndemo: patch\n---\n\nPatch.\n").expect("frag");
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("cargo generate-lockfile"));
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
