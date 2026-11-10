//! The `release asset` command, run through the binary. It covers package
//! tarballs, built as before, and vendored sets beside them. `--dry-run`
//! builds and uploads nothing.

#![expect(
    clippy::expect_used,
    reason = "the fixture helpers fail fast, as in tests/common.rs"
)]

mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use indoc::indoc;
use tempfile::TempDir;

fn run(root: &Path, args: &[&str]) -> Output {
    common::verctl()
        .current_dir(root)
        .args(args)
        .output()
        .expect("spawn")
}

fn json(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

/// The entries of the gzipped tarball at `path`, as `tar` lists them.
fn entries(path: &Path) -> Vec<String> {
    let output = Command::new("tar")
        .arg("tzf")
        .arg(path)
        .output()
        .expect("spawn tar");
    assert!(output.status.success(), "tar tzf {}", path.display());
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A package config from before vendored sets existed. Its build does nothing
/// over a binary already in place, so only the packing is under test.
#[test]
fn a_package_target_plans_and_packs_as_before() {
    let root = TempDir::new().expect("tmp");
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml
            assets:
              build: ["true"]
              binary: out/{bin}
              linux-x64: {}
        "#},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.2.3"
        "#},
    )
    .expect("cargo");
    fs::create_dir_all(root.path().join("out")).expect("out");
    fs::write(root.path().join("out/demo"), "binary").expect("binary");
    let report = json(&run(
        root.path(),
        &[
            "release",
            "asset",
            "linux-x64",
            "--dry-run",
            "--format",
            "json",
        ],
    ));
    assert_eq!(
        report,
        serde_json::json!({
            "bin": "demo",
            "version": "1.2.3",
            "tag": "v1.2.3",
            "has_assets": true,
            "matrix": {
                "include": [{
                    "id": "linux-x64",
                    "machine": "ubuntu-latest",
                    "labels": ["ubuntu-latest"],
                    "os": "linux",
                    "arch": "x64",
                    "triple": "x86_64-unknown-linux-gnu",
                    "asset": "demo_1.2.3_linux_x64.tar.gz"
                }]
            },
            "tarball": "./demo_1.2.3_linux_x64.tar.gz",
            "uploaded": null
        })
    );
    assert_eq!(
        entries(&root.path().join("demo_1.2.3_linux_x64.tar.gz")),
        ["demo"]
    );
}

/// No command verctl starts inherits a registry token's variable: only the
/// publisher gets the token, under its own name. The build writes the
/// environment it sees.
#[test]
fn a_build_inherits_no_registry_token() {
    let root = TempDir::new().expect("tmp");
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml
            assets:
              build: ["sh", "-c", "env > seen"]
              binary: out/{bin}
              linux-x64: {}
        "#},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.2.3"
        "#},
    )
    .expect("cargo");
    fs::create_dir_all(root.path().join("out")).expect("out");
    fs::write(root.path().join("out/demo"), "binary").expect("binary");
    let output = common::verctl()
        .current_dir(root.path())
        .env("VERCTL_REGISTRY_TOKEN", "from-secret")
        .env("VERCTL_TRUSTED_PUBLISHING_TOKEN", "from-trusted")
        .env("BUILD_SEES", "this")
        .args([
            "release",
            "asset",
            "linux-x64",
            "--dry-run",
            "--format",
            "json",
        ])
        .output()
        .expect("spawn");
    json(&output);
    let seen = fs::read_to_string(root.path().join("seen")).expect("seen");
    assert!(seen.contains("BUILD_SEES=this"), "{seen}");
    assert!(!seen.contains("VERCTL_REGISTRY_TOKEN"), "{seen}");
    assert!(!seen.contains("VERCTL_TRUSTED_PUBLISHING_TOKEN"), "{seen}");
    assert!(!seen.contains("from-"), "{seen}");
}

/// A package whose release is two executables, a service unit, and a licence,
/// all already in place under the triple's directory.
fn files_root() -> TempDir {
    let root = TempDir::new().expect("tmp");
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
            assets:
              build: ["true"]
              dir: out/{triple}
              files: [demo, demod, "demod@.service", LICENSE]
              linux-x64: {}
        "#},
    );
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.2.3"
        "#},
    )
    .expect("cargo");
    let out = root.path().join("out/x86_64-unknown-linux-gnu");
    fs::create_dir_all(&out).expect("out");
    for file in ["demo", "demod", "demod@.service", "LICENSE"] {
        fs::write(out.join(file), file).expect("file");
    }
    root
}

#[test]
fn a_package_target_packs_the_listed_files() {
    let root = files_root();
    let report = json(&run(
        root.path(),
        &[
            "release",
            "asset",
            "linux-x64",
            "--dry-run",
            "--format",
            "json",
        ],
    ));
    assert_eq!(report["tarball"], "./demo_1.2.3_linux_x64.tar.gz");
    assert_eq!(
        entries(&root.path().join("demo_1.2.3_linux_x64.tar.gz")),
        ["demo", "demod", "demod@.service", "LICENSE"]
    );
}

/// A `--dry-run` written before `asset` takes effect for the step. The root is
/// no repository and holds no token, so an upload would fail the run.
#[test]
fn a_dry_run_before_asset_uploads_nothing() {
    let root = files_root();
    let report = json(&run(
        root.path(),
        &["release", "-n", "asset", "linux-x64", "--format", "json"],
    ));
    assert_eq!(report["tarball"], "./demo_1.2.3_linux_x64.tar.gz");
    assert!(report["uploaded"].is_null(), "{report}");
}

#[test]
fn a_package_file_missing_after_the_build_is_named() {
    let root = files_root();
    fs::remove_file(
        root.path()
            .join("out/x86_64-unknown-linux-gnu/demod@.service"),
    )
    .expect("remove");
    let output = run(root.path(), &["release", "asset", "linux-x64", "--dry-run"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("demod@.service is missing"), "{stderr}");
}

/// [`files_root`], with `linux-x64` listing its own files in place of the
/// shared list.
fn target_files_root() -> TempDir {
    let root = files_root();
    common::write_config(
        root.path(),
        indoc! {r#"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
            assets:
              build: ["true"]
              dir: out/{triple}
              files: [demo, demod, "demod@.service", LICENSE]
              linux-x64:
                files: [demo, LICENSE]
        "#},
    );
    root
}

#[test]
fn a_target_packs_its_own_files() {
    let root = target_files_root();
    json(&run(
        root.path(),
        &[
            "release",
            "asset",
            "linux-x64",
            "--dry-run",
            "--format",
            "json",
        ],
    ));
    assert_eq!(
        entries(&root.path().join("demo_1.2.3_linux_x64.tar.gz")),
        ["demo", "LICENSE"]
    );
}

#[test]
fn a_target_file_missing_after_the_build_names_its_list() {
    let root = target_files_root();
    fs::remove_file(root.path().join("out/x86_64-unknown-linux-gnu/LICENSE")).expect("remove");
    let output = run(root.path(), &["release", "asset", "linux-x64", "--dry-run"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        stderr.contains("assets.linux-x64.files: ") && stderr.contains("LICENSE is missing"),
        "{stderr}"
    );
}

const VENDORED: &str = indoc! {r"
    runners:
      linux:
        labels: [ubuntu-24.04]
      macos:
        labels: [macos-15]
    vendored:
      libexample:
        version: 1.4.0
        dir: out/{id}
        files: [lib/libexample.a, include, LICENSE]
        targets:
          linux-x64:
            runner: linux
          macos-arm64:
            runner: macos
"};

fn vendored_root() -> TempDir {
    let root = TempDir::new().expect("tmp");
    common::write_config(root.path(), VENDORED);
    let out = root.path().join("out/linux-x64");
    fs::create_dir_all(out.join("lib")).expect("lib");
    fs::create_dir_all(out.join("include/example")).expect("include");
    fs::write(out.join("lib/libexample.a"), "archive").expect("lib");
    fs::write(out.join("include/example/vt.h"), "header").expect("header");
    fs::write(out.join("LICENSE"), "MIT").expect("license");
    root
}

/// A repository that releases no package of its own still vendors, and the
/// variable selects the set as the flag would.
#[test]
fn a_vendored_target_packs_the_listed_files() {
    let root = vendored_root();
    let output = common::verctl()
        .current_dir(root.path())
        .args([
            "release",
            "asset",
            "linux-x64",
            "--dry-run",
            "--format",
            "json",
        ])
        .env("VERCTL_VENDORED", "libexample")
        .output()
        .expect("spawn");
    let report = json(&output);
    assert_eq!(report["tag"], "libexample-1.4.0");
    assert_eq!(report["version"], "1.4.0");
    assert_eq!(
        report["targets"][1]["asset"],
        "libexample-1.4.0-macos-arm64.tar.gz"
    );
    assert_eq!(report["targets"][1]["labels"][0], "macos-15");
    assert_eq!(report["archive"], "./libexample-1.4.0-linux-x64.tar.gz");
    let mut listed = entries(&root.path().join("libexample-1.4.0-linux-x64.tar.gz"));
    listed.sort();
    assert_eq!(
        listed,
        [
            "LICENSE",
            "include/",
            "include/example/",
            "include/example/vt.h",
            "lib/libexample.a"
        ]
    );
}

#[test]
fn a_vendored_file_missing_after_the_build_is_named() {
    let root = vendored_root();
    fs::remove_file(root.path().join("out/linux-x64/LICENSE")).expect("remove");
    let output = run(
        root.path(),
        &[
            "release",
            "asset",
            "linux-x64",
            "--dry-run",
            "--vendored",
            "libexample",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("LICENSE is missing after the build"),
        "{stderr}"
    );
}

/// `status --vendored` writes the set's asset outputs in the packages' shape
/// and nothing else. It needs no token, fetches no history, and looks up no
/// release. No token is set and the GitHub API address is closed, so any of
/// those would fail the run.
#[test]
fn a_vendored_set_writes_the_same_step_outputs() {
    let root = vendored_root();
    common::init(root.path());
    common::commit_all(root.path(), "chore: vendor");
    let out = root.path().join("github-output");
    let output = common::verctl()
        .current_dir(root.path())
        .env("GITHUB_REPOSITORY", "acme/demo")
        .env("GITHUB_API_URL", "http://127.0.0.1:9")
        .args([
            "status",
            "--vendored",
            "libexample",
            "--github-output",
            out.to_str().expect("utf-8"),
        ])
        .output()
        .expect("spawn");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(&out).expect("output"),
        indoc! {r#"
            has_assets=true
            tag=libexample-1.4.0
            matrix={"include":[{"id":"linux-x64","labels":["ubuntu-24.04"]},{"id":"macos-arm64","labels":["macos-15"]}]}
        "#}
    );
}

#[test]
fn a_vendored_set_declares_its_own_tag() {
    let root = vendored_root();
    let output = run(
        root.path(),
        &[
            "release",
            "asset",
            "linux-x64",
            "--vendored",
            "libexample",
            "--tag",
            "v1.0.0",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--tag"), "{stderr}");
}

#[test]
fn an_undeclared_vendored_set_lists_the_declared_ones() {
    let root = vendored_root();
    let output = run(
        root.path(),
        &["release", "asset", "linux-x64", "--vendored", "nope"],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("declared: libexample"), "{stderr}");
}
