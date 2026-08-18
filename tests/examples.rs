//! `examples/verctl.toml` is documentation that has to keep parsing. It is a
//! real config, not a commented one, so the schema is what validates it.

use indoc::indoc;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use verctl::config::Config;
use verctl::{assets, ci, pins};

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/verctl.toml")
}

/// The example declares manifests it does not ship. Give it the tree it
/// describes so both planners can run against the file as written.
#[allow(clippy::expect_used)]
fn planted() -> TempDir {
    let root = TempDir::new().expect("tmp");
    fs::copy(example(), root.path().join("verctl.toml")).expect("copy");
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "verctl"
            version = "4.5.6"
        "#},
    )
    .expect("cargo");
    fs::write(
        root.path().join("package.json"),
        indoc! {r#"
            { "name": "@org/pkg", "version": "4.5.6" }
        "#},
    )
    .expect("npm");
    fs::write(root.path().join("VERSION"), "4.5.6\n").expect("plain");
    root
}

#[test]
fn the_example_config_parses() {
    let config = Config::load(&example()).expect("the shipped example must parse");
    let names: Vec<&str> = config.packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["verctl", "@org/pkg", "py-tool"]);
    assert_eq!(config.runners.len(), 2, "two machines are declared");
    assert_eq!(config.publishers.len(), 1, "twine is declared");
    assert_eq!(config.pins.len(), 1, "one collocated pin is declared");
}

/// Resolving each declared table by hand would skip the stock merge production
/// applies, so a valid `[drivers.npm] after = "…"` — stock supplying the rest —
/// would fail here while working in the field. Plan the publish instead: it is
/// the path that resolves every driver, reads the version through it, and
/// resolves every publisher. Every table the example declares is used by a
/// package, so nothing documented escapes this.
#[test]
fn the_example_resolves_every_driver_and_publisher_it_declares() {
    let root = planted();
    let config = Config::load(&root.path().join("verctl.toml")).expect("parse");
    let plan = verctl::publish::plan(&config, root.path()).expect("plan the publish");
    let resolved: Vec<(&str, &str, &str, Vec<&str>)> = plan
        .packages
        .iter()
        .map(|entry| {
            (
                entry.name.as_str(),
                entry.version.as_str(),
                entry.noun.as_str(),
                entry.argv.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        resolved,
        [
            (
                "verctl",
                "4.5.6",
                "crate",
                vec![
                    "cargo",
                    "publish",
                    "--locked",
                    "--manifest-path",
                    root.path().join("Cargo.toml").to_str().expect("utf-8"),
                ],
            ),
            // `registry = "github"` is what puts --registry here.
            (
                "@org/pkg",
                "4.5.6",
                "package",
                vec![
                    "bun",
                    "publish",
                    "--tolerate-republish",
                    "--cwd",
                    root.path().to_str().expect("utf-8"),
                    "--registry",
                    "https://npm.pkg.github.com",
                ],
            ),
            // The `plain` driver read this version off stdin, and `twine` is
            // the declared publisher rendering it.
            ("py-tool", "4.5.6", "package", vec!["uv", "publish"]),
        ]
    );
}

/// The comment above `[drivers.plain]` promises both directions: stdin is the
/// file, stdout is the version on read and the whole new file on write, and a
/// write also gets `VERCTL_VERSION`. Run both — an argv that opens its own file
/// or ignores the variable would document a protocol that does not exist.
#[test]
fn the_example_command_driver_reads_and_writes_through_stdio() {
    let config = Config::load(&example()).expect("parse");
    let spec = config.drivers["plain"].clone();
    let driver = spec.into_driver("plain").expect("resolve");
    assert_eq!(
        driver.read("4.5.6\n").expect("read the manifest on stdin"),
        "4.5.6"
    );
    assert_eq!(
        driver
            .write("4.5.6\n", "5.0.0")
            .expect("write the whole new file to stdout"),
        "5.0.0\n"
    );
}

#[test]
fn the_example_ci_table_plans_the_checks_it_documents() {
    let config = Config::load(&example()).expect("parse");
    let plan = ci::plan(&config).expect("plan");
    let checks: Vec<(&str, String)> = plan
        .matrix
        .include
        .iter()
        .map(|job| (job.name.as_str(), job.labels.join(", ")))
        .collect();
    assert_eq!(
        checks,
        [
            ("audit", "nscloud-ubuntu-24.04-amd64-4x8".to_owned()),
            ("verify (ns)", "nscloud-ubuntu-24.04-amd64-4x8".to_owned()),
            ("verify (big)", "self-hosted, linux, x64".to_owned()),
        ],
        "the comments above [ci] promise exactly these checks"
    );
}

#[test]
fn the_example_assets_table_plans_the_targets_it_documents() {
    let root = planted();
    let config = Config::load(&root.path().join("verctl.toml")).expect("parse");
    let plan = assets::plan(&config, root.path()).expect("plan");
    let targets: Vec<(&str, String, &str, &str)> = plan
        .matrix
        .include
        .iter()
        .map(|target| {
            (
                target.id.as_str(),
                target.labels.join(", "),
                target.asset.as_str(),
                // The triple is what `cargo build --target` gets. Omit it here
                // and the example could document a wrong one silently.
                target.triple.as_str(),
            )
        })
        .collect();
    assert_eq!(
        targets,
        [
            (
                "darwin-arm64",
                "macos-latest".to_owned(),
                "verctl_4.5.6_macos_arm64.tar.gz",
                "aarch64-apple-darwin"
            ),
            (
                "linux-arm64",
                "nscloud-ubuntu-24.04-amd64-4x8".to_owned(),
                "verctl_4.5.6_linux_arm64.tar.gz",
                "aarch64-unknown-linux-gnu"
            ),
            (
                "linux-x64",
                "self-hosted, linux, x64".to_owned(),
                "verctl_4.5.6_linux_x64.tar.gz",
                "x86_64-unknown-linux-gnu"
            ),
        ],
        "a built-in machine, a declared one, and a user-defined platform"
    );
}

/// `config.pins.len()` proves a pin was parsed, not that it points anywhere.
/// Run the rewrite: a wrong `file` cannot be read, a wrong `tool` is rejected,
/// and a `package` naming nothing is skipped without touching the file.
#[test]
fn the_example_pin_rewrites_the_file_it_names() {
    let root = planted();
    let config = Config::load(&root.path().join("verctl.toml")).expect("parse");
    fs::write(
        root.path().join("mise.release.toml"),
        indoc! {r#"
            [tools]
            "github:victor-software-house/verctl" = "0.0.1"

            [task_config]
            includes = [
              "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.1",
            ]
        "#},
    )
    .expect("pin file");
    let versions = pins::current_versions(root.path(), &config).expect("read versions");
    let written = pins::write(root.path(), &config.pins, &versions).expect("rewrite the pin");
    assert_eq!(written, [root.path().join("mise.release.toml")]);
    let body = fs::read_to_string(root.path().join("mise.release.toml")).expect("reread");
    assert_eq!(
        body,
        indoc! {r#"
            [tools]
            "github:victor-software-house/verctl" = "4.5.6"

            [task_config]
            includes = [
              "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v4.5.6",
            ]
        "#},
        "the tool pin and the ?ref= move together, and nothing else moves"
    );
}
