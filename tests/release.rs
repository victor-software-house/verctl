use indoc::indoc;
use std::fs;
use tempfile::TempDir;
use verctl::fragment::{self, Bump, Fragment, PackageBump};
use verctl::prepare::PlanEntry;
use verctl::release;

#[test]
fn contributing_fragments_skip_unused() {
    let used = Fragment {
        path: std::path::PathBuf::from("used.md"),
        packages: vec![PackageBump {
            name: "demo".into(),
            bump: Bump::Patch,
        }],
        summary: "used".into(),
    };
    let unused = Fragment {
        path: std::path::PathBuf::from("skip.md"),
        packages: vec![PackageBump {
            name: "other".into(),
            bump: Bump::Patch,
        }],
        summary: "skip".into(),
    };
    let plan = [PlanEntry {
        name: "demo".into(),
        from: "1.0.0".into(),
        to: "1.0.1".into(),
        bump: Bump::Patch,
        path: std::path::PathBuf::from("Cargo.toml"),
        driver: verctl::driver::Driver::Path {
            format: verctl::driver::Format::Toml,
            keys: vec!["package.version".into()],
            after: None,
        },
    }];
    let fragments = [used, unused];
    let hits = release::contributing_fragments(&plan, &fragments);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].summary, "used");
}

#[test]
fn consume_fragments_deletes_the_files() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("gone.md");
    fs::write(&path, "x").unwrap();
    let fragment = Fragment {
        path: path.clone(),
        packages: vec![PackageBump {
            name: "demo".into(),
            bump: Bump::Patch,
        }],
        summary: "gone".into(),
    };
    release::consume_fragments(&[fragment]).unwrap();
    assert!(!path.exists());
}

#[test]
fn prepend_changelog_creates_and_inserts() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("CHANGELOG.md");
    release::prepend_changelog(
        &path,
        indoc! {"
            ## demo 1.0.1

            - first

        "},
    )
    .unwrap();
    let first = fs::read_to_string(&path).unwrap();
    assert!(
        first.starts_with(indoc! {"
            # Changelog

            ## demo 1.0.1
        "}),
        "{first}"
    );
    release::prepend_changelog(
        &path,
        indoc! {"
            ## demo 1.0.2

            - second

        "},
    )
    .unwrap();
    let next = fs::read_to_string(&path).unwrap();
    assert!(next.contains("## demo 1.0.2"), "{next}");
    assert!(
        next.find("1.0.2").unwrap() < next.find("1.0.1").unwrap(),
        "{next}"
    );
}

#[test]
fn write_changelogs_renders_fragment_summaries() {
    let root = TempDir::new().unwrap();
    let changelog = root.path().join("CHANGELOG.md");
    let frag_path = root.path().join("note.md");
    let fragment = fragment::parse_str(
        indoc! {"
            ---
            demo: patch
            ---

            Local prepare writes versions.
        "},
        &frag_path,
    )
    .unwrap();
    let plan = [PlanEntry {
        name: "demo".into(),
        from: "1.0.0".into(),
        to: "1.0.1".into(),
        bump: Bump::Patch,
        path: root.path().join("Cargo.toml"),
        driver: verctl::driver::Driver::Path {
            format: verctl::driver::Format::Toml,
            keys: vec!["package.version".into()],
            after: None,
        },
    }];
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .unwrap();
    let config = verctl::config::Config::load(&root.path().join("verctl.toml")).unwrap();
    release::write_changelogs(&config, root.path(), &plan, &[fragment]).unwrap();
    let body = fs::read_to_string(&changelog).unwrap();
    assert!(body.contains("## demo 1.0.1"), "{body}");
    assert!(body.contains("Local prepare writes versions"), "{body}");
}

#[test]
fn each_package_gets_its_own_changelog() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("crates/a")).unwrap();
    fs::create_dir_all(root.path().join("crates/b")).unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "a"
            path = "crates/a/Cargo.toml"
            [[packages]]
            name = "b"
            path = "crates/b/Cargo.toml"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("crates/a/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "a"
            version = "1.0.0"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("crates/b/Cargo.toml"),
        indoc! {r#"
            [package]
            name = "b"
            version = "2.0.0"
        "#},
    )
    .unwrap();
    let changes = root.path().join(".changeset");
    fs::create_dir_all(&changes).unwrap();
    fs::write(
        changes.join("both.md"),
        indoc! {"
            ---
            a: patch
            b: minor
            ---

            Two crates.
        "},
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["prepare", "--no-pr"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let a = fs::read_to_string(root.path().join("crates/a/CHANGELOG.md")).unwrap();
    let b = fs::read_to_string(root.path().join("crates/b/CHANGELOG.md")).unwrap();
    assert!(a.contains("## a 1.0.1"), "{a}");
    assert!(b.contains("## b 2.1.0"), "{b}");
    assert!(!a.contains("## b "), "{a}");
    assert!(!b.contains("## a "), "{b}");
}

#[test]
fn notes_for_reads_each_package_changelog() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("crates/a")).unwrap();
    fs::create_dir_all(root.path().join("crates/b")).unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "a"
            path = "crates/a/Cargo.toml"
            [[packages]]
            name = "b"
            path = "crates/b/Cargo.toml"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("crates/a/CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## a 1.0.1

            - First crate.

            ## a 1.0.0

            - Birth.
        "},
    )
    .unwrap();
    fs::write(
        root.path().join("crates/b/CHANGELOG.md"),
        indoc! {"
            # Changelog

            ## b 2.1.0

            - Second crate.
        "},
    )
    .unwrap();
    let config = verctl::config::Config::load(&root.path().join("verctl.toml")).unwrap();
    let notes = release::notes_for(&config, root.path(), [("a", "1.0.1"), ("b", "2.1.0")]);
    assert!(notes.contains("## a 1.0.1"), "{notes}");
    assert!(notes.contains("First crate"), "{notes}");
    assert!(notes.contains("## b 2.1.0"), "{notes}");
    assert!(!notes.contains("## a 1.0.0"), "{notes}");
}

#[test]
fn missing_changelog_section_fails_closed() {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .unwrap();
    let config = verctl::config::Config::load(&root.path().join("verctl.toml")).unwrap();
    let err =
        release::require_changelog_versions(&config, root.path(), [("demo", "1.0.1")]).unwrap_err();
    assert!(format!("{err:#}").contains("demo@1.0.1"), "{err:#}");
}

#[test]
fn pr_body_is_the_changelog() {
    assert_eq!(release::pr_body(""), "Prepared by verctl.");
    let notes = indoc! {"
        ## demo 1.0.1

        - Patch.
    "};
    assert_eq!(release::pr_body(notes), notes.trim());
}

#[test]
fn named_or_bare_heading_covers_the_version() {
    assert!(
        release::changelog_section_for(
            indoc! {"
                # Changelog

                ## demo 1.0.1

                - x
            "},
            "demo",
            "1.0.1",
        )
        .is_some()
    );
    assert!(
        release::changelog_section_for(
            indoc! {"
                # Changelog

                ## 1.0.1

                - x
            "},
            "demo",
            "1.0.1",
        )
        .is_some()
    );
    assert!(
        release::changelog_section_for(
            indoc! {"
                # Changelog

                ## demo 1.0.0

                - x
            "},
            "demo",
            "1.0.1",
        )
        .is_none()
    );
}

#[test]
fn local_prepare_writes_changelog_and_consumes() {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .unwrap();
    let changes = root.path().join(".changeset");
    fs::create_dir_all(&changes).unwrap();
    let frag = changes.join("bump.md");
    fs::write(
        &frag,
        indoc! {"
            ---
            demo: patch
            ---

            Patch.
        "},
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_verctl"))
        .current_dir(root.path())
        .args(["prepare", "--no-pr"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !frag.exists(),
        "prepare consumes fragments after writing CHANGELOG"
    );
    let log = fs::read_to_string(root.path().join("CHANGELOG.md")).unwrap();
    assert!(log.contains("# Changelog"), "{log}");
    assert!(log.contains("## demo 1.0.1"), "{log}");
    assert!(log.contains("Patch."), "{log}");
}
