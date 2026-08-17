//! `[prepare].after` and unexpected dirty paths.
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
