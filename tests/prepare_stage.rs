//! What `prepare` is allowed to write: `[prepare].after`, `[[pins]]`,
//! and unexpected dirty paths.
#![allow(missing_docs, clippy::unwrap_used)]

use git2::{Repository, Signature};
use indoc::indoc;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn init_repo(root: &Path) {
    let repo = Repository::init(root).unwrap();
    let sig = Signature::now("t", "t@example.com").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tid = index.write_tree().unwrap();
    let tree = repo.find_tree(tid).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();
}

fn write_min(root: &Path, extra: &str) {
    fs::write(
        root.join("verctl.toml"),
        format!(
            "{}\n{extra}",
            indoc! {r#"
                [[packages]]
                name = "demo"
                path = "Cargo.toml"
            "#}
        ),
    )
    .unwrap();
    fs::write(
        root.join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .unwrap();
    fs::create_dir_all(root.join(".changeset")).unwrap();
    fs::write(
        root.join(".changeset/one.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Bump.
        "},
    )
    .unwrap();
}

fn prepare(root: &Path) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root)
        .args(["prepare", "--no-pr"])
        .output()
        .unwrap()
}

#[test]
fn after_may_write_declared_globs() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            [prepare]
            after = ["touch", "src/version.rs"]
            stage = ["src/version.rs"]
        "#},
    );
    fs::create_dir_all(root.path().join("src")).unwrap();
    init_repo(root.path());
    let output = prepare(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.path().join("src/version.rs").exists());
}

#[test]
fn after_deleted_staged_file_is_collected() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            [prepare]
            after = ["rm", "src/gone.rs"]
            stage = ["src/gone.rs"]
        "#},
    );
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/gone.rs"), "old\n").unwrap();
    init_repo(root.path());
    let output = prepare(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.path().join("src/gone.rs").exists());
}

#[test]
fn after_unexpected_path_fails() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            [prepare]
            after = ["touch", "secret.env"]
        "#},
    );
    init_repo(root.path());
    let output = prepare(root.path());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("secret.env"), "{stderr}");
    assert!(stderr.contains("[prepare].stage"), "{stderr}");
}

#[test]
fn bunfig_is_passed_as_config() {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "@org/pkg"
            path = "package.json"
            registry = "github"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "0.0.1" }
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("bunfig.toml"),
        indoc! {r#"
            [install.scopes]
            "@org" = { url = "https://npm.pkg.github.com", token = "$GITHUB_TOKEN" }
        "#},
    )
    .unwrap();
    let planned = verctl::publish::plan(
        &verctl::config::Config::load(&root.path().join("verctl.toml")).unwrap(),
        root.path(),
    )
    .unwrap();
    let argv = &planned.packages[0].argv;
    assert!(argv.contains(&"--config".into()), "{argv:?}");
    assert!(
        argv.iter().any(|part| part.ends_with("bunfig.toml")),
        "{argv:?}"
    );
    assert!(!argv.contains(&"--registry".into()), "{argv:?}");
}

/// The tag names the Version PR commit, so a pin rewritten after publish
/// never reaches the tree a consumer fetches by ref. `prepare` writes it.
#[test]
fn pins_move_with_the_bump() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            [[pins]]
            file = "consumer/mise.toml"
            tool = "github:acme/demo"
            package = "demo"
        "#},
    );
    fs::create_dir_all(root.path().join("consumer")).unwrap();
    fs::write(
        root.path().join("consumer/mise.toml"),
        indoc! {r#"
            [tools]
            "github:acme/demo" = "1.0.0"

            [task_config]
            includes = [
              "git::https://github.com/acme/demo.git//tasks/demo?ref=v1.0.0",
            ]
        "#},
    )
    .unwrap();
    init_repo(root.path());
    let output = prepare(root.path());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(
        fs::read_to_string(root.path().join("consumer/mise.toml")).unwrap(),
        indoc! {r#"
            [tools]
            "github:acme/demo" = "1.0.1"

            [task_config]
            includes = [
              "git::https://github.com/acme/demo.git//tasks/demo?ref=v1.0.1",
            ]
        "#},
        "the tool pin and the ?ref= both name the version being released"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("consumer/mise.toml"), "{stdout}");
}

/// A rewritten pin is part of the release, not a surprise in the worktree.
/// Without that, the dirty check would reject the file `prepare` just wrote.
#[test]
fn a_rewritten_pin_is_not_unexpected_dirt() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            [[pins]]
            file = "pinned.toml"
            tool = "github:acme/demo"
            package = "demo"
        "#},
    );
    fs::write(
        root.path().join("pinned.toml"),
        indoc! {r#"
            [tools]
            "github:acme/demo" = "1.0.0"
        "#},
    )
    .unwrap();
    init_repo(root.path());
    let output = prepare(root.path());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(!stderr.contains("[prepare].stage"), "{stderr}");
}

#[test]
fn a_pin_naming_no_configured_package_is_left_alone() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            [[pins]]
            file = "pinned.toml"
            tool = "github:acme/ghost"
            package = "ghost"
        "#},
    );
    let body = indoc! {r#"
        [tools]
        "github:acme/ghost" = "1.0.0"
    "#};
    fs::write(root.path().join("pinned.toml"), body).unwrap();
    init_repo(root.path());
    let output = prepare(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.path().join("pinned.toml")).unwrap(),
        body,
        "no package named it, so nothing claims the file"
    );
}

#[test]
fn a_dry_run_reports_the_pin_without_writing_it() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            [[pins]]
            file = "pinned.toml"
            tool = "github:acme/demo"
            package = "demo"
        "#},
    );
    let body = indoc! {r#"
        [tools]
        "github:acme/demo" = "1.0.0"
    "#};
    fs::write(root.path().join("pinned.toml"), body).unwrap();
    init_repo(root.path());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["prepare", "--no-pr", "--dry-run"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("pinned.toml"), "{stdout}");
    assert_eq!(
        fs::read_to_string(root.path().join("pinned.toml")).unwrap(),
        body,
        "a dry run names the file it would rewrite and rewrites nothing"
    );
}
