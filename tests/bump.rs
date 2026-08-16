use indoc::indoc;
use std::fs;
use tempfile::TempDir;
use verctl::bump::{apply, read_version, write_version};
use verctl::config::{Config, ManifestKind};
use verctl::fragment::{Bump, parse_str};
use verctl::prepare;

#[test]
fn patch_and_minor() {
    assert_eq!(apply("1.2.3", Bump::Patch).expect("p"), "1.2.4");
    assert_eq!(apply("1.2.3", Bump::Minor).expect("m"), "1.3.0");
    assert_eq!(apply("0.0.22", Bump::Patch).expect("0p"), "0.0.23");
    assert_eq!(apply("0.1.4", Bump::Minor).expect("0m"), "0.2.0");
}

#[test]
fn rejects_major_on_0x() {
    let error = apply("0.0.22", Bump::Major).expect_err("major");
    assert!(format!("{error:#}").contains("0.x"), "{error:#}");
}

#[test]
fn major_on_1x() {
    assert_eq!(apply("1.4.2", Bump::Major).expect("1"), "2.0.0");
}

#[test]
fn cargo_workspace_keeps_comments() {
    let raw = indoc! {r#"
        # keep me
        [workspace.package]
        version = "0.0.21" # pin
        edition = "2024"

        [workspace]
        members = ["crates/app"]
    "#};
    let next = write_version(ManifestKind::Cargo, raw, "0.0.22").expect("write");
    assert!(next.contains("# keep me"), "{next}");
    assert!(next.contains("edition = \"2024\""), "{next}");
    assert!(!next.contains("members = [\"other\"]"), "{next}");
    assert_eq!(
        read_version(ManifestKind::Cargo, &next).expect("read"),
        "0.0.22"
    );
}

#[test]
fn cargo_package_table() {
    let raw = indoc! {r#"
        [package]
        name = "demo"
        version = "1.0.0"
    "#};
    let next = write_version(ManifestKind::Cargo, raw, "1.0.1").expect("write");
    assert_eq!(
        read_version(ManifestKind::Cargo, &next).expect("read"),
        "1.0.1"
    );
    assert!(next.contains("name = \"demo\""));
}

#[test]
fn npm_replaces_only_the_version_string() {
    let raw = indoc! {r#"
        {
          "name": "@scope/pkg",
          "version": "2.0.0",
          "private": true
        }
    "#};
    let next = write_version(ManifestKind::Npm, raw, "2.1.0").expect("write");
    assert_eq!(
        next,
        indoc! {r#"
            {
              "name": "@scope/pkg",
              "version": "2.1.0",
              "private": true
            }
        "#}
    );
}

#[test]
fn prepare_fails_closed_on_unknown_package() {
    let root = TempDir::new().expect("tmp");
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "known"
            path = "Cargo.toml"
        "#},
    )
    .expect("cfg");
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "known"
            version = "0.1.0"
        "#},
    )
    .expect("cargo");
    let config = Config::load(&root.path().join("verctl.toml")).expect("load");
    let fragment = parse_str(
        indoc! {"
            ---
            ghost: patch
            ---

            Nope.
        "},
        "ghost.md",
    )
    .expect("frag");
    let error = prepare::plan(&config, &[fragment], root.path()).expect_err("unknown");
    assert!(format!("{error:#}").contains("ghost"), "{error:#}");
}

#[test]
fn prepare_applies_max_bump_across_fragments() {
    let root = TempDir::new().expect("tmp");
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .expect("cfg");
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.2.3"
        "#},
    )
    .expect("cargo");
    let config = Config::load(&root.path().join("verctl.toml")).expect("load");
    let patch = parse_str(
        indoc! {"
            ---
            demo: patch
            ---

            Fix.
        "},
        "a.md",
    )
    .expect("p");
    let minor = parse_str(
        indoc! {"
            ---
            demo: minor
            ---

            Feat.
        "},
        "b.md",
    )
    .expect("m");
    let plan = prepare::plan(&config, &[patch, minor], root.path()).expect("plan");
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].from, "1.2.3");
    assert_eq!(plan[0].to, "1.3.0");
    let follow = prepare::apply_plan(&plan).expect("apply");
    assert_eq!(follow, ["cargo generate-lockfile"]);
    let after = fs::read_to_string(root.path().join("Cargo.toml")).expect("read");
    assert!(after.contains("version = \"1.3.0\""));
}
