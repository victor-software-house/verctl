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
