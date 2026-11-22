//! What `version` is allowed to write: `prepare.after`, lockfile owners,
//! `pins`, consumed fragments, and unexpected dirty paths.
#![allow(
    clippy::unwrap_used,
    reason = "fixture setup unwraps; a broken fixture fails the test"
)]

mod common;

use std::fs;
use std::path::Path;
use std::process::Output;

use indoc::{formatdoc, indoc};
use tempfile::TempDir;
use verctl::config::Config;
use verctl::{config, publish};

fn init_repo(root: &Path) {
    common::init(root);
    common::commit_all(root, "init");
}

fn write_min(root: &Path, extra: &str) {
    common::write_config(
        root,
        &format!(
            "{}\n{extra}",
            indoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
            "}
        ),
    );
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

fn prepare(root: &Path) -> Output {
    common::verctl()
        .current_dir(root)
        .args(["version", "--no-pr"])
        .output()
        .unwrap()
}

#[test]
fn after_may_write_declared_globs() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {"
            prepare:
              after: [touch, src/version.rs]
              stage: [src/version.rs]
        "},
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
        indoc! {"
            prepare:
              after: [rm, src/gone.rs]
              stage: [src/gone.rs]
        "},
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
        indoc! {"
            prepare:
              after: [touch, secret.env]
        "},
    );
    init_repo(root.path());
    let output = prepare(root.path());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("secret.env"), "{stderr}");
    assert!(stderr.contains("prepare.stage"), "{stderr}");
}

#[test]
fn bunfig_is_passed_as_config() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: "@org/pkg"
                path: package.json
                registry: github
        "#},
    );
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
    let planned = publish::plan(
        &Config::load(&root.path().join(config::FILE)).unwrap(),
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
/// never reaches the tree a consumer fetches by ref. `version` writes it.
#[test]
fn pins_move_with_the_bump() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            pins:
              - file: consumer/mise.toml
                tool: "github:acme/demo"
                package: demo
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

/// A rewritten pin is part of the release and no surprise in the worktree.
/// Without that, the dirty check would reject the file `version` wrote.
#[test]
fn a_rewritten_pin_is_not_unexpected_dirt() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            pins:
              - file: pinned.toml
                tool: "github:acme/demo"
                package: demo
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
    assert!(!stderr.contains("prepare.stage"), "{stderr}");
}

#[test]
fn a_pin_naming_no_configured_package_is_left_alone() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            pins:
              - file: pinned.toml
                tool: "github:acme/ghost"
                package: ghost
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
            pins:
              - file: pinned.toml
                tool: "github:acme/demo"
                package: demo
        "#},
    );
    let body = indoc! {r#"
        [tools]
        "github:acme/demo" = "1.0.0"
    "#};
    fs::write(root.path().join("pinned.toml"), body).unwrap();
    init_repo(root.path());
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "--no-pr", "--dry-run"])
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

/// Pins move with the bump, not with whatever a manifest happens to say.
/// A pin left one release behind on purpose is not this release's business.
#[test]
fn a_pin_on_a_package_no_fragment_bumped_stays_where_it_is() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml
              - name: other
                path: other/Cargo.toml

            pins:
              - file: pinned.toml
                tool: "github:acme/other"
                package: other
        "#},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .unwrap();
    fs::create_dir_all(root.path().join("other")).unwrap();
    fs::write(
        root.path().join("other/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "other"
            version = "3.0.0"
        "#},
    )
    .unwrap();
    let lagging = indoc! {r#"
        [tools]
        "github:acme/other" = "2.0.0"
    "#};
    fs::write(root.path().join("pinned.toml"), lagging).unwrap();
    fs::create_dir_all(root.path().join(".changeset")).unwrap();
    fs::write(
        root.path().join(".changeset/one.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Bump.
        "},
    )
    .unwrap();
    init_repo(root.path());
    let output = prepare(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.path().join("pinned.toml")).unwrap(),
        lagging,
        "no fragment named `other`, so its pin is not this release's business"
    );
}

/// A pin that cannot be rewritten is caught before anything is written.
/// Otherwise prepare stops with versions bumped, no changelog, and the
/// fragments still on disk.
#[test]
fn a_broken_pin_fails_before_any_manifest_moves() {
    let root = TempDir::new().unwrap();
    write_min(
        root.path(),
        indoc! {r#"
            pins:
              - file: pinned.toml
                tool: "github:acme/demo"
                package: demo
        "#},
    );
    fs::write(root.path().join("pinned.toml"), "# no [tools] table\n").unwrap();
    init_repo(root.path());
    let output = prepare(root.path());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("pinned.toml"), "{stderr}");
    assert!(
        fs::read_to_string(root.path().join("Cargo.toml"))
            .unwrap()
            .contains("version = \"1.0.0\""),
        "the manifest must not move when a pin cannot be rewritten"
    );
    assert!(
        root.path().join(".changeset/one.md").exists(),
        "the fragment must survive a failed prepare"
    );
}

/// What a pre-commit hook exports for its own git: `GIT_INDEX_FILE` names a
/// temporary index relative to the repository root, beside `GIT_DIR` and
/// `GIT_WORK_TREE`.
const HOOK_ENV: [(&str, &str); 3] = [
    ("GIT_INDEX_FILE", ".git/hook-index"),
    ("GIT_DIR", ".git"),
    ("GIT_WORK_TREE", "."),
];

fn verctl(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    common::verctl()
        .current_dir(root)
        .args(args)
        .envs(env.iter().copied())
        .output()
        .unwrap()
}

/// verctl reads the repository on disk and ignores the variables a hook
/// exports. The hook's index here is empty, so honouring it would leave the
/// committed template untracked and fail the run. The index keeps its bytes,
/// whether verctl reads it through the template plan or through status, and
/// the dry run leaves `git status` clean.
#[test]
fn prepare_ignores_hook_variables() {
    let root = TempDir::new().unwrap();
    write_min(root.path(), "");
    fs::create_dir_all(root.path().join(".ctl/templates")).unwrap();
    fs::write(
        root.path().join(".ctl/templates/served.txt.jinja"),
        indoc! {r#"
            demo {{ versions["demo"] }}
        "#},
    )
    .unwrap();
    init_repo(root.path());
    common::git(
        root.path(),
        &["read-tree", "--empty", "--index-output=.git/hook-index"],
    );
    let index = fs::read(root.path().join(".git/index")).unwrap();
    let dry_run = ["version", "--no-pr", "--dry-run", "--color", "never"];
    let plain = verctl(root.path(), &dry_run, &[]);
    assert!(
        plain.status.success(),
        "{}",
        String::from_utf8_lossy(&plain.stderr)
    );
    let hooked = verctl(root.path(), &dry_run, &HOOK_ENV);
    assert!(
        hooked.status.success(),
        "{}",
        String::from_utf8_lossy(&hooked.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&hooked.stdout),
        String::from_utf8_lossy(&plain.stdout)
    );
    assert_eq!(fs::read(root.path().join(".git/index")).unwrap(), index);
    assert_eq!(common::git(root.path(), &["status", "--porcelain"]), "");
    let index = fs::read(root.path().join(".git/index")).unwrap();
    let applied = verctl(root.path(), &["version", "--no-pr"], &HOOK_ENV);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(fs::read(root.path().join(".git/index")).unwrap(), index);
    assert!(root.path().join("served.txt").exists());
}

/// A crate at 0.3.0 with a tracked `Cargo.lock`, a pending `demo: patch`, and
/// `extra` appended to the config.
fn locked_crate(root: &Path, extra: &str) {
    common::write_config(
        root,
        &format!(
            "{}\n{extra}",
            indoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
            "}
        ),
    );
    fs::write(
        root.join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.3.0"
            edition = "2024"
        "#},
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "").unwrap();
    fs::write(
        root.join("Cargo.lock"),
        indoc! {r#"
            # This file is automatically @generated by Cargo.
            # It is not intended for manual editing.
            version = 4

            [[package]]
            name = "demo"
            version = "0.3.0"
        "#},
    )
    .unwrap();
    fs::create_dir_all(root.join(".changeset")).unwrap();
    fs::write(
        root.join(".changeset/fix.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Fix.
        "},
    )
    .unwrap();
    init_repo(root);
}

/// Runs `version --no-pr --format json` offline and returns its report.
fn version_json(root: &Path) -> serde_json::Value {
    let output = common::verctl()
        .current_dir(root)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["version", "--no-pr", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn the_version_step_refreshes_a_tracked_cargo_lock() {
    let root = TempDir::new().unwrap();
    locked_crate(root.path(), "");
    let report = version_json(root.path());
    assert_eq!(
        report["after"],
        serde_json::json!(["cargo update --workspace"])
    );
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).unwrap();
    assert!(manifest.contains(r#"version = "0.3.1""#), "{manifest}");
    let lock = fs::read_to_string(root.path().join("Cargo.lock")).unwrap();
    assert!(lock.contains(r#"version = "0.3.1""#), "{lock}");
}

#[test]
fn prepare_after_that_repeats_the_lockfile_owner_runs_once() {
    let root = TempDir::new().unwrap();
    locked_crate(
        root.path(),
        indoc! {"
            prepare:
              after: [cargo, update, --workspace]
              stage: [Cargo.lock]
        "},
    );
    let report = version_json(root.path());
    assert_eq!(
        report["after"],
        serde_json::json!(["cargo update --workspace"])
    );
    let lock = fs::read_to_string(root.path().join("Cargo.lock")).unwrap();
    assert!(lock.contains(r#"version = "0.3.1""#), "{lock}");
}

#[test]
fn a_fragment_that_bumps_nothing_is_consumed() {
    let root = TempDir::new().unwrap();
    write_min(root.path(), "");
    fs::remove_file(root.path().join(".changeset/one.md")).unwrap();
    fs::write(
        root.path().join(".changeset/docs.md"),
        indoc! {"
            ---
            demo: none
            ---

            Docs only.
        "},
    )
    .unwrap();
    init_repo(root.path());
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).unwrap();
    let output = prepare(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.path().join(".changeset/docs.md").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("Cargo.toml")).unwrap(),
        manifest
    );
}

const BUN_PACKAGE: &str = indoc! {r#"
    {
      "name": "kairos",
      "version": "0.1.0",
      "private": true
    }
"#};

/// The root workspace of a `bun.lock` records no `version` key, and the package
/// has a private dependency that the owner's install would fetch.
const BUN_LOCK_ROOT_WITHOUT_VERSION: &str = indoc! {r#"
    {
      "lockfileVersion": 1,
      "workspaces": {
        "": {
          "name": "kairos",
          "devDependencies": {
            "@acme/private": "^1.0.0",
          },
        },
      },
      "packages": {
        "@acme/private": ["@acme/private@1.0.0", "", {}, "sha512-x"],
      },
    }
"#};

const BUN_LOCK_ROOT_WITH_VERSION: &str = indoc! {r#"
    {
      "lockfileVersion": 1,
      "workspaces": {
        "": {
          "name": "kairos",
          "version": "0.1.0",
        },
      },
      "packages": {},
    }
"#};

const UV_PYPROJECT: &str = indoc! {r#"
    [project]
    name = "My_Pkg"
    version = "0.2.0"
"#};

/// uv writes the project name normalised, `my-pkg` for `My_Pkg`.
const UV_LOCK: &str = indoc! {r#"
    version = 1
    requires-python = ">=3.12"

    [[package]]
    name = "my-pkg"
    version = "0.2.0"
    source = { virtual = "." }
"#};

const WORKSPACE_ROOT: &str = indoc! {r#"
    [workspace]
    members = ["member"]
    resolver = "3"

    [workspace.package]
    version = "1.0.0"
    edition = "2024"
"#};

const WORKSPACE_MEMBER: &str = indoc! {r#"
    [package]
    name = "member"
    version.workspace = true
    edition.workspace = true
"#};

const WORKSPACE_LOCK: &str = indoc! {r#"
    # This file is automatically @generated by Cargo.
    # It is not intended for manual editing.
    version = 4

    [[package]]
    name = "member"
    version = "1.0.0"
"#};

/// A committed repository that holds `files`, a config naming package `name` at
/// the first of them, and a pending patch for it.
fn repo_with(root: &Path, name: &str, files: &[(&str, &str)]) {
    common::write_config(
        root,
        &formatdoc! {"
            packages:
              - name: {name}
                path: {path}
        ", path = files[0].0},
    );
    for (path, body) in files {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, body).unwrap();
    }
    fs::create_dir_all(root.join(".changeset")).unwrap();
    fs::write(
        root.join(".changeset/fix.md"),
        formatdoc! {"
            ---
            {name}: patch
            ---

            Fix.
        "},
    )
    .unwrap();
    init_repo(root);
}

/// The commands `version --dry-run` lists, which are the ones a real run would
/// start.
fn planned_after(root: &Path) -> Vec<String> {
    let output = common::verctl()
        .current_dir(root)
        .args(["version", "--no-pr", "--dry-run", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    serde_json::from_value(report["after"].clone()).unwrap()
}

#[test]
fn a_bun_lock_whose_root_records_no_version_runs_no_owner() {
    let root = TempDir::new().unwrap();
    repo_with(
        root.path(),
        "kairos",
        &[
            ("package.json", BUN_PACKAGE),
            ("bun.lock", BUN_LOCK_ROOT_WITHOUT_VERSION),
        ],
    );
    let report = version_json(root.path());
    assert_eq!(report["after"], serde_json::json!([]));
    assert_eq!(
        fs::read_to_string(root.path().join("bun.lock")).unwrap(),
        BUN_LOCK_ROOT_WITHOUT_VERSION
    );
    let status = common::git(root.path(), &["status", "--porcelain"]);
    assert!(!status.contains("bun.lock"), "{status}");
}

#[test]
fn a_bun_lock_whose_root_records_a_version_still_runs_bun_install() {
    let root = TempDir::new().unwrap();
    repo_with(
        root.path(),
        "kairos",
        &[
            ("package.json", BUN_PACKAGE),
            ("bun.lock", BUN_LOCK_ROOT_WITH_VERSION),
        ],
    );
    assert_eq!(planned_after(root.path()), ["bun install"]);
}

#[test]
fn a_uv_lock_that_normalises_the_project_name_still_runs_uv_lock() {
    let root = TempDir::new().unwrap();
    repo_with(
        root.path(),
        "My_Pkg",
        &[("pyproject.toml", UV_PYPROJECT), ("uv.lock", UV_LOCK)],
    );
    assert_eq!(planned_after(root.path()), ["uv lock"]);
}

#[test]
fn a_cargo_workspace_version_still_runs_cargo_update() {
    let root = TempDir::new().unwrap();
    repo_with(
        root.path(),
        "demo",
        &[
            ("Cargo.toml", WORKSPACE_ROOT),
            ("member/Cargo.toml", WORKSPACE_MEMBER),
            ("Cargo.lock", WORKSPACE_LOCK),
        ],
    );
    assert_eq!(planned_after(root.path()), ["cargo update --workspace"]);
}
