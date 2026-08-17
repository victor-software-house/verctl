//! `examples/verctl.toml` is documentation that has to keep parsing. It is a
//! real config, not a commented one, so the schema is what validates it.

use indoc::indoc;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use verctl::config::Config;
use verctl::{assets, ci};

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
    root
}

#[test]
fn the_example_config_parses() {
    let config = Config::load(&example()).expect("the shipped example must parse");
    let names: Vec<&str> = config.packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["verctl", "@org/pkg"]);
    assert_eq!(config.runners.len(), 2, "two machines are declared");
    assert_eq!(config.publishers.len(), 1, "twine is declared");
    assert_eq!(config.pins.len(), 1, "one collocated pin is declared");
}

/// Parsing a `[drivers]` table is not resolving it: `into_driver` runs only for
/// drivers a package uses, so a declared-but-unused one can ship broken.
#[test]
fn every_example_driver_resolves() {
    let config = Config::load(&example()).expect("parse");
    assert_eq!(
        config.drivers.len(),
        3,
        "cargo, npm, and plain are declared"
    );
    for (name, spec) in &config.drivers {
        spec.clone()
            .into_driver(name)
            .unwrap_or_else(|err| panic!("driver {name:?} in the example is invalid: {err:#}"));
    }
}

/// The comment above `[drivers.plain]` promises stdin in and the version out.
/// Run it, because an argv that opens its own file would document a contract
/// the driver protocol does not have.
#[test]
fn the_example_command_driver_reads_stdin() {
    let config = Config::load(&example()).expect("parse");
    let spec = config.drivers["plain"].clone();
    let driver = spec.into_driver("plain").expect("resolve");
    let version = driver.read("4.5.6\n").expect("read the manifest on stdin");
    assert_eq!(version, "4.5.6");
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
    let targets: Vec<(&str, String, &str)> = plan
        .matrix
        .include
        .iter()
        .map(|target| {
            (
                target.id.as_str(),
                target.labels.join(", "),
                target.asset.as_str(),
            )
        })
        .collect();
    assert_eq!(
        targets,
        [
            (
                "darwin-arm64",
                "macos-latest".to_owned(),
                "verctl_4.5.6_macos_arm64.tar.gz"
            ),
            (
                "linux-arm64",
                "nscloud-ubuntu-24.04-amd64-4x8".to_owned(),
                "verctl_4.5.6_linux_arm64.tar.gz"
            ),
            (
                "linux-x64",
                "self-hosted, linux, x64".to_owned(),
                "verctl_4.5.6_linux_x64.tar.gz"
            ),
        ],
        "a built-in machine, a declared one, and a user-defined platform"
    );
}
