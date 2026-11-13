//! `check` against served files and pins, and `render` as the command that
//! fixes what `check` refuses. Both run through the binary in a real
//! repository.

mod common;

use std::fs;
use std::path::Path;
use std::process::Output;

use indoc::indoc;
use tempfile::TempDir;

const TEMPLATE: &str = ".ctl/templates/notes.md.jinja";
const SERVED: &str = "notes.md";

/// A repository at `demo` 0.1.0 that serves `notes.md` from a template and
/// pins `tools.toml`, with every file committed and matching.
fn repo() -> TempDir {
    let root = TempDir::new().expect("tempdir");
    common::init(root.path());
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml

            patterns:
              tool:
                match: "demo@{version}"
                occurrences: once
                whole_line: true

            pins:
              - file: tools.toml
                package: demo
                patterns: [tool]
        "#},
    );
    write(
        root.path(),
        "Cargo.toml",
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.1.0"
        "#},
    );
    write(
        root.path(),
        TEMPLATE,
        indoc! {r#"
            Install demo {{ versions["demo"] }}.
        "#},
    );
    write(root.path(), SERVED, "Install demo 0.1.0.\n");
    write(root.path(), "tools.toml", "demo@0.1.0\n");
    fs::create_dir_all(root.path().join(".changeset")).expect("changeset dir");
    common::git(root.path(), &["add", "-A"]);
    common::git(root.path(), &["commit", "-q", "-m", "init"]);
    root
}

fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    fs::write(path, body).expect("write");
}

fn read(root: &TempDir, relative: &str) -> String {
    fs::read_to_string(root.path().join(relative)).expect("read")
}

fn verctl(root: &TempDir, args: &[&str]) -> Output {
    common::verctl()
        .current_dir(root.path())
        .args(args)
        .args(["--color", "never"])
        .output()
        .expect("spawn")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn check_passes_when_every_served_file_matches() {
    let root = repo();
    let output = verctl(&root, &["check"]);
    assert!(output.status.success(), "{}", stderr(&output));
}

/// A hand edit fails `check` with no fragment pending, and `render` puts the
/// rendered bytes back.
#[test]
fn render_reverts_a_hand_edited_served_file() {
    let root = repo();
    write(root.path(), SERVED, "Install demo 9.9.9.\n");
    let output = verctl(&root, &["check"]);
    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains(SERVED), "{message}");
    assert!(message.contains(TEMPLATE), "{message}");
    assert!(message.contains("verctl render"), "{message}");

    let output = verctl(&root, &["render"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(read(&root, SERVED), "Install demo 0.1.0.\n");
    let output = verctl(&root, &["check"]);
    assert!(output.status.success(), "{}", stderr(&output));
}

/// A template edited with no fragment pending fails `check` until `render`
/// writes its output.
#[test]
fn a_template_edit_fails_check_until_render() {
    let root = repo();
    write(
        root.path(),
        TEMPLATE,
        indoc! {r#"
            Install demo {{ versions["demo"] }} with mise.
        "#},
    );
    let output = verctl(&root, &["check"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains(SERVED), "{}", stderr(&output));

    let output = verctl(&root, &["render", "--dry-run"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(read(&root, SERVED), "Install demo 0.1.0.\n");

    let output = verctl(&root, &["render"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(read(&root, SERVED), "Install demo 0.1.0 with mise.\n");
    let output = verctl(&root, &["check"]);
    assert!(output.status.success(), "{}", stderr(&output));
}

/// A pin left at another version, and one whose pattern does not match, both
/// fail `check` and are named.
#[test]
fn check_names_a_stale_pin() {
    let root = repo();
    write(root.path(), "tools.toml", "demo@0.0.9\n");
    let output = verctl(&root, &["check"]);
    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains("tools.toml"), "{message}");
    assert!(message.contains("other than demo 0.1.0"), "{message}");

    write(root.path(), "tools.toml", "nothing pinned here\n");
    let message = stderr(&verctl(&root, &["check"]));
    assert!(message.contains("matches 0 times"), "{message}");
}

/// A file a template renders and a `pins` entry rewrites has two owners:
/// `version` refuses before it writes anything, and `check` and `render`
/// refuse too.
#[test]
fn a_file_with_two_owners_is_refused_before_any_write() {
    let root = repo();
    write(
        root.path(),
        ".ctl/templates/tools.toml.jinja",
        "demo@{{ versions[\"demo\"] }}\n",
    );
    common::git(root.path(), &["add", "-A"]);
    write(
        root.path(),
        ".changeset/fix.md",
        indoc! {"
            ---
            demo: patch
            ---

            Fix.
        "},
    );
    for verb in [&["version"][..], &["check"], &["render"]] {
        let output = verctl(&root, verb);
        assert!(!output.status.success(), "{verb:?}");
        let message = stderr(&output);
        assert!(
            message.contains("tools.toml has two owners"),
            "{verb:?}: {message}"
        );
        assert!(
            message.contains(".ctl/templates/tools.toml.jinja"),
            "{verb:?}: {message}"
        );
        assert!(message.contains("package demo"), "{verb:?}: {message}");
    }
    assert!(read(&root, "Cargo.toml").contains("version = \"0.1.0\""));
    assert_eq!(read(&root, "tools.toml"), "demo@0.1.0\n");
}

/// A candidate where a pin's version goes is a version verctl wrote, so
/// `version` moves it with the manifest and the pinned file holds the new
/// version.
#[test]
fn a_prerelease_pin_moves_with_the_version() {
    let root = repo();
    write(root.path(), "tools.toml", "demo@0.1.0-rc.1\n");
    common::git(root.path(), &["commit", "-qam", "prerelease"]);
    write(
        root.path(),
        ".changeset/fix.md",
        indoc! {"
            ---
            demo: patch
            ---

            Fix.
        "},
    );
    let output = verctl(&root, &["version"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(read(&root, "tools.toml"), "demo@0.1.1\n");
    assert!(read(&root, "Cargo.toml").contains("version = \"0.1.1\""));
}
