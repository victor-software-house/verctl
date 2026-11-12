//! Release candidates through `verctl version`, in a git history built the
//! way Version PRs build it: each candidate's Version PR consumes its
//! fragments, and the stable version after them lists every one.

mod common;

use std::fs;
use std::path::Path;
use std::process::Output;

use indoc::{formatdoc, indoc};
use tempfile::TempDir;

/// The demo package, with `setting` as an extra line under it, such as
/// `prerelease: rc`, or none.
fn configure(root: &Path, setting: &str) {
    common::write_config(
        root,
        &formatdoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                {setting}
        "},
    );
}

fn manifest(version: &str) -> String {
    formatdoc! {r#"
        [package]
        name = "demo"
        version = "{version}"
    "#}
}

/// A patch fragment for demo with `summary` as its entry.
fn fragment(root: &Path, name: &str, summary: &str) {
    let dir = root.join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join(name),
        formatdoc! {"
            ---
            demo: patch
            ---

            {summary}
        "},
    )
    .expect("fragment");
}

/// `verctl version --no-pr` in `root`.
fn version(root: &Path) -> Output {
    common::verctl()
        .current_dir(root)
        .args(["version", "--no-pr"])
        .output()
        .expect("run")
}

/// Runs `verctl version`, requires it to pass, and commits what it wrote as
/// the Version PR's merge would.
fn release(root: &Path, subject: &str) {
    let output = version(root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    common::commit_all(root, subject);
}

fn read(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(name)).expect("read")
}

/// A repository at demo 0.8.1, released, with an empty `.changeset/`.
fn released_repo(setting: &str) -> TempDir {
    let root = TempDir::new().expect("tmp");
    common::init(root.path());
    configure(root.path(), setting);
    fs::write(root.path().join("Cargo.toml"), manifest("0.8.1")).expect("cargo");
    fs::create_dir_all(root.path().join(".changeset")).expect("dir");
    fs::write(root.path().join(".changeset/README.md"), "Fragments.\n").expect("readme");
    common::commit_all(root.path(), "chore: release demo 0.8.1");
    root
}

/// Two candidates each consume one fragment, then the key goes, and the
/// stable section lists both while the candidate sections stay below it.
#[test]
fn two_candidates_then_stable_lists_both_in_the_stable_section() {
    let repo = released_repo("prerelease: rc");
    let root = repo.path();
    fragment(root, "new-config.md", "Read the new config.");
    common::commit_all(root, "feat: read the new config (#1)");
    release(root, "chore: release demo 0.8.2-rc.1 (#2)");
    fragment(root, "fix-tag.md", "Fix the tag.");
    common::commit_all(root, "fix: the tag (#3)");
    release(root, "chore: release demo 0.8.2-rc.2 (#4)");
    configure(root, "");
    common::commit_all(root, "chore: release demo stable (#5)");
    release(root, "chore: release demo 0.8.2 (#6)");
    assert!(read(root, "Cargo.toml").contains(r#"version = "0.8.2""#));
    assert_eq!(
        read(root, "CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## demo 0.8.2

            - Read the new config.
            - Fix the tag.

            ## demo 0.8.2-rc.2

            - Fix the tag.

            ## demo 0.8.2-rc.1

            - Read the new config.
        "}
    );
}

/// The first candidate under a new tag, with nothing pending, lists what the
/// earlier candidates carried instead of an empty section.
#[test]
fn a_new_tag_with_no_fragment_lists_the_earlier_candidates() {
    let repo = released_repo("prerelease: beta");
    let root = repo.path();
    fragment(root, "one.md", "Add one.");
    common::commit_all(root, "feat: one (#1)");
    release(root, "chore: release demo 0.8.2-beta.1 (#2)");
    fragment(root, "two.md", "Add two.");
    common::commit_all(root, "feat: two (#3)");
    release(root, "chore: release demo 0.8.2-beta.2 (#4)");
    configure(root, "prerelease: rc");
    common::commit_all(root, "chore: move demo to rc (#5)");
    release(root, "chore: release demo 0.8.2-rc.1 (#6)");
    assert!(read(root, "Cargo.toml").contains(r#"version = "0.8.2-rc.1""#));
    assert!(
        read(root, "CHANGELOG.md").starts_with(indoc! {"
            # Changelog

            ## demo 0.8.2-rc.1

            - Add one.
            - Add two.

            ## demo 0.8.2-beta.2
        "}),
        "{}",
        read(root, "CHANGELOG.md")
    );
}

/// A pending fragment that reuses the file name of one a candidate consumed
/// keeps its own link, and so does the consumed one.
#[test]
fn a_reused_fragment_name_links_each_entry_to_its_own_pull_request() {
    let repo = released_repo("prerelease: rc");
    let root = repo.path();
    common::git(
        root,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/demo.git",
        ],
    );
    fragment(root, "fix.md", "Fix the old bug.");
    common::commit_all(root, "fix: the old bug (#1)");
    release(root, "chore: release demo 0.8.2-rc.1 (#2)");
    configure(root, "");
    fragment(root, "fix.md", "Fix the new bug.");
    common::commit_all(root, "fix: the new bug (#3)");
    release(root, "chore: release demo 0.8.2 (#4)");
    assert!(
        read(root, "CHANGELOG.md").starts_with(indoc! {"
            # Changelog

            ## demo 0.8.2

            - Fix the old bug ([#1](https://github.com/acme/demo/pull/1)).
            - Fix the new bug ([#3](https://github.com/acme/demo/pull/3)).
        "}),
        "{}",
        read(root, "CHANGELOG.md")
    );
}

/// A shallow clone that ends before a candidate's Version PR would build a
/// short stable section, so `version` fails, naming the commit, and writes
/// nothing.
#[test]
fn a_shallow_clone_fails_before_any_file_changes() {
    let repo = released_repo("prerelease: rc");
    let root = repo.path();
    fragment(root, "one.md", "Add one.");
    common::commit_all(root, "feat: one (#1)");
    release(root, "chore: release demo 0.8.2-rc.1 (#2)");
    configure(root, "");
    let boundary = common::commit_all(root, "chore: release demo stable (#3)");
    fs::write(root.join(".git/shallow"), format!("{boundary}\n")).expect("shallow");
    let cargo = read(root, "Cargo.toml");
    let changelog = read(root, "CHANGELOG.md");
    let output = version(root);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains(&boundary), "{stderr}");
    assert_eq!(read(root, "Cargo.toml"), cargo);
    assert_eq!(read(root, "CHANGELOG.md"), changelog);
}
