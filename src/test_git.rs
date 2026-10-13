//! Fixture repositories built with the `git` binary.

use std::path::Path;
use std::process::Command;

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
