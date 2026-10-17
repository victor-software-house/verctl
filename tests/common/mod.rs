#![allow(
    dead_code,
    clippy::expect_used,
    reason = "each test binary uses its own subset of these fixture helpers, which fail fast"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use indoc::indoc;
use tempfile::TempDir;

/// Write a repo's declarations where verctl looks for them, and hand back the
/// path so a test can pass it to `-c` or `Config::load` without spelling the
/// layout again.
pub(crate) fn write_config(root: &Path, yaml: &str) -> PathBuf {
    let path = root.join(verctl::config::FILE);
    fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    fs::write(&path, yaml).expect("cfg");
    path
}

pub(crate) fn demo_root() -> TempDir {
    let root = TempDir::new().expect("tmp");
    write_demo(root.path());
    root
}

pub(crate) fn write_demo(root: &Path) {
    write_config(
        root,
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(
        root.join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .expect("cargo");
    let changes = root.join(".changeset");
    fs::create_dir_all(&changes).expect("dir");
    fs::write(
        changes.join("bump.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Patch.
        "},
    )
    .expect("frag");
}

/// The verctl binary under test, with the agent-shell colour variables and
/// every `VERCTL_*` variable removed, so the suite passes in any shell and
/// every flag comes from argv unless a test sets its variable back on top.
pub(crate) fn verctl() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_verctl"));
    for name in [
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "FORCE_COLOR",
        "NO_COLOR",
        "COLORTERM",
    ] {
        command.env_remove(name);
    }
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("VERCTL_") {
            command.env_remove(name);
        }
    }
    command
}

/// Run `git` with `args` in `dir` under a fixed identity and no user or system
/// configuration, and return its trimmed stdout. A failure panics with stderr.
pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git stdout is utf-8")
        .trim()
        .to_owned()
}

/// Initialise a repository in `dir` with `main` as its initial branch.
pub(crate) fn init(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
}

/// Commit everything in `dir` on top of HEAD, with `subject`, and return the
/// new commit's id. An unchanged tree still commits.
pub(crate) fn commit_all(dir: &Path, subject: &str) -> String {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", subject]);
    git(dir, &["rev-parse", "HEAD"])
}
