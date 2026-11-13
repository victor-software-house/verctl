//! Fragment consumption, changelog writing, and Version PR notes, including
//! pull request and commit links.

mod common;

use std::fs;
use std::path::PathBuf;

use indoc::{formatdoc, indoc};
use tempfile::TempDir;
use verctl::config::Config;
use verctl::driver::{Driver, Format};
use verctl::fragment::{self, Bump, Fragment, Group, PackageBump};
use verctl::prepare::PlanEntry;
use verctl::{config, release};

#[test]
fn contributing_fragments_skip_unused() {
    let used = Fragment {
        path: PathBuf::from("used.md"),
        packages: vec![PackageBump {
            name: "demo".into(),
            bump: Bump::Patch,
            breaking: false,
        }],
        summary: "used".into(),
        group: None,
    };
    let unused = Fragment {
        path: PathBuf::from("skip.md"),
        packages: vec![PackageBump {
            name: "other".into(),
            bump: Bump::Patch,
            breaking: false,
        }],
        summary: "skip".into(),
        group: None,
    };
    let plan = [PlanEntry {
        name: "demo".into(),
        from: "1.0.0".into(),
        to: "1.0.1".into(),
        bump: Bump::Patch,
        path: PathBuf::from("Cargo.toml"),
        driver: Driver::Path {
            format: Format::Toml,
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
            breaking: false,
        }],
        summary: "gone".into(),
        group: None,
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
        driver: Driver::Path {
            format: Format::Toml,
            keys: vec!["package.version".into()],
            after: None,
        },
    }];
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    let config = Config::load(&root.path().join(config::FILE)).unwrap();
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
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: a
                path: crates/a/Cargo.toml
              - name: b
                path: crates/b/Cargo.toml
        "},
    );
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
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "--no-pr"])
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
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: a
                path: crates/a/Cargo.toml
              - name: b
                path: crates/b/Cargo.toml
        "},
    );
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
    let config = Config::load(&root.path().join(config::FILE)).unwrap();
    let notes = release::notes_for(&config, root.path(), [("a", "1.0.1"), ("b", "2.1.0")]);
    assert!(notes.contains("## a 1.0.1"), "{notes}");
    assert!(notes.contains("First crate"), "{notes}");
    assert!(notes.contains("## b 2.1.0"), "{notes}");
    assert!(!notes.contains("## a 1.0.0"), "{notes}");
}

#[test]
fn missing_changelog_section_fails_closed() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    let config = Config::load(&root.path().join(config::FILE)).unwrap();
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

const CUT_NOTE_END: &str = "in this pull request's diff.";

fn sections(count: usize, entry: &str) -> String {
    (1..=count)
        .map(|n| {
            formatdoc! {"
                ## demo 1.0.{n}

                - {entry}
            "}
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn kept_before_note(body: &str) -> &str {
    let Some((kept, note)) = body.rsplit_once("\n\n") else {
        panic!("a cut body ends with the note, got: {body}");
    };
    assert!(note.ends_with(CUT_NOTE_END), "{note}");
    kept
}

#[test]
fn pr_body_at_the_limit_is_unchanged() {
    let heading = "## demo 1.0.1 ";
    let pad = release::PR_BODY_LIMIT - heading.chars().count();
    let notes = format!("{heading}{}", "é".repeat(pad));
    assert_eq!(notes.chars().count(), release::PR_BODY_LIMIT);
    assert_eq!(release::pr_body(&notes), notes);
}

#[test]
fn pr_body_over_the_limit_keeps_the_bullets_that_fit() {
    let bullets = (1..=2_000)
        .map(|n| format!("- change {n}, written at some length"))
        .collect::<Vec<_>>()
        .join("\n");
    let notes = formatdoc! {"
        ## demo 1.0.1

        ### Patch Changes

        {bullets}
    "};
    assert!(notes.chars().count() > release::PR_BODY_LIMIT);
    let body = release::pr_body(&notes);
    assert!(body.chars().count() <= release::PR_BODY_LIMIT);
    let kept = kept_before_note(&body);
    assert!(kept.starts_with("## demo 1.0.1\n\n### Patch Changes\n\n- change 1,"));
    assert!(notes.starts_with(kept));
    assert!(notes[kept.len()..].starts_with("\n- change "));
    assert!(kept.chars().count() > release::PR_BODY_LIMIT - 200);
}

#[test]
fn pr_body_cut_drops_a_heading_left_at_the_end() {
    let notes = format!(
        "{}{}",
        indoc! {"
            - one

            ### Late heading

            - "},
        "a".repeat(release::PR_BODY_LIMIT)
    );
    let body = release::pr_body(&notes);
    assert_eq!(kept_before_note(&body), "- one");
}

#[test]
fn pr_body_cut_keeps_whole_multibyte_characters() {
    let notes = sections(2_000, "日本語のメモ 🦀 é ü, written at some length");
    assert!(notes.chars().count() > release::PR_BODY_LIMIT);
    let body = release::pr_body(&notes);
    assert!(body.chars().count() <= release::PR_BODY_LIMIT);
    let kept = kept_before_note(&body);
    assert!(notes.starts_with(kept));
    assert!(kept.ends_with("written at some length"));
}

#[test]
fn pr_body_cut_never_lands_inside_a_fenced_code_block() {
    let code = formatdoc! {"
        # not a heading
        let kept = false;
    "}
    .repeat(release::PR_BODY_LIMIT / 20);
    let notes = formatdoc! {"
        ## demo 1.0.2

        - Patch.

        ## demo 1.0.1

        ```
        {code}```
    "};
    let body = release::pr_body(&notes);
    assert!(body.chars().count() <= release::PR_BODY_LIMIT);
    assert_eq!(
        kept_before_note(&body),
        indoc! {"
            ## demo 1.0.2

            - Patch."}
    );
}

#[test]
fn pr_body_with_no_safe_cut_is_only_the_note() {
    let notes = "a".repeat(release::PR_BODY_LIMIT + 1);
    let body = release::pr_body(&notes);
    assert!(body.starts_with("The rest of the changelog"), "{body}");
    assert!(body.ends_with(CUT_NOTE_END), "{body}");
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
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
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
    let output = common::verctl()
        .current_dir(root.path())
        .args(["version", "--no-pr"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !frag.exists(),
        "version consumes fragments after writing CHANGELOG"
    );
    let log = fs::read_to_string(root.path().join("CHANGELOG.md")).unwrap();
    assert!(log.contains("# Changelog"), "{log}");
    assert!(log.contains("## demo 1.0.1"), "{log}");
    assert!(log.contains("Patch."), "{log}");
}

/// A squash-merged fragment links its pull request, and one pushed straight
/// to the branch links its commit.
#[test]
fn prepare_links_the_pull_request_or_commit_that_added_each_fragment() {
    let root = TempDir::new().unwrap();
    common::write_config(
        root.path(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
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
    common::init(root.path());
    common::git(
        root.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/demo.git",
        ],
    );
    common::commit_all(root.path(), "chore: start");
    let changes = root.path().join(".changeset");
    fs::create_dir_all(&changes).unwrap();
    fs::write(
        changes.join("parser.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Fix the parser
        "},
    )
    .unwrap();
    common::commit_all(root.path(), "fix: the parser (#12)");
    fs::write(
        changes.join("direct.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Fix the reader
        "},
    )
    .unwrap();
    let direct = common::commit_all(root.path(), "fix: the reader");
    let output = common::verctl()
        .current_dir(root.path())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .args(["version", "--no-pr"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(root.path().join("CHANGELOG.md")).unwrap();
    assert!(
        log.contains("- Fix the parser ([#12](https://github.com/acme/demo/pull/12))."),
        "{log}"
    );
    let short = &direct[..7];
    assert!(
        log.contains(&format!(
            "- Fix the reader ([`{short}`](https://github.com/acme/demo/commit/{direct}))."
        )),
        "{log}"
    );
}

/// With no `GITHUB_REF_NAME`, no `VERCTL_DEFAULT_BRANCH`, and no
/// `origin/HEAD`, `version --pr` fails before it writes anything instead of
/// opening the Version PR against `main`.
#[test]
fn prepare_pr_without_a_known_base_fails_before_writing() {
    let root = common::demo_root();
    common::init(root.path());
    common::git(
        root.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/demo.git",
        ],
    );
    common::commit_all(root.path(), "chore: start");
    let output = common::verctl()
        .current_dir(root.path())
        .env("GITHUB_TOKEN", "unused")
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_REF_NAME")
        .env_remove("VERCTL_DEFAULT_BRANCH")
        .args(["version", "--pr"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("cannot tell which branch"), "{stderr}");
    assert!(
        root.path().join(".changeset/bump.md").exists(),
        "the fragment stays"
    );
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).unwrap();
    assert!(manifest.contains(r#"version = "1.0.0""#), "{manifest}");
}

fn demo_plan() -> [PlanEntry; 1] {
    [PlanEntry {
        name: "demo".into(),
        from: "1.0.0".into(),
        to: "1.0.1".into(),
        bump: Bump::Patch,
        path: PathBuf::from("Cargo.toml"),
        driver: Driver::Path {
            format: Format::Toml,
            keys: vec!["package.version".into()],
            after: None,
        },
    }]
}

fn demo_fragment(summary: &str, group: Option<Group>) -> Fragment {
    Fragment {
        path: PathBuf::from(format!("{summary}.md")),
        packages: vec![PackageBump {
            name: "demo".into(),
            bump: Bump::Patch,
            breaking: false,
        }],
        summary: summary.into(),
        group,
    }
}

/// A fragment with a second paragraph, a list, and a code fence stays one
/// entry. Its wrapped first paragraph takes the period after its last line.
/// Later lines are indented under the bullet and blank lines stay blank. The
/// next fragment starts the next item of the same list.
///
/// The fixtures are a recorded fragment and the section it renders.
#[test]
fn a_multi_paragraph_fragment_stays_inside_its_entry() {
    let fragment = fragment::parse_str(
        include_str!("testdata/changelog/multi-paragraph.fragment.md"),
        "multi-paragraph.md",
    )
    .unwrap();
    let fragments = [fragment, demo_fragment("Second", None)];
    assert_eq!(
        release::changelog_sections(&demo_plan(), &fragments).unwrap(),
        include_str!("testdata/changelog/multi-paragraph.section.md")
    );
}

#[test]
fn a_section_with_no_group_stays_flat() {
    let fragments = [demo_fragment("First", None), demo_fragment("Second", None)];
    assert_eq!(
        release::changelog_sections(&demo_plan(), &fragments).unwrap(),
        indoc! {"
            ## demo 1.0.1

            - First.
            - Second.

        "}
    );
}

#[test]
fn an_ungrouped_fragment_lands_in_the_default_group() {
    let fragments = [
        demo_fragment("Fix the parser", Some(Group::Fixed)),
        demo_fragment("Rename a flag", None),
        demo_fragment("Add a verb", Some(Group::Added)),
    ];
    assert_eq!(
        release::changelog_sections(&demo_plan(), &fragments).unwrap(),
        indoc! {"
            ## demo 1.0.1

            ### Added

            - Add a verb.

            ### Changed

            - Rename a flag.

            ### Fixed

            - Fix the parser.

        "}
    );
}

#[test]
fn a_patch_bang_entry_comes_first_under_breaking() {
    let mut breaking = demo_fragment("Replace a public field", None);
    breaking.packages[0].breaking = true;
    let fragments = [breaking, demo_fragment("Fix the parser", None)];
    assert_eq!(
        release::changelog_sections(&demo_plan(), &fragments).unwrap(),
        indoc! {"
            ## demo 1.0.1

            ### Breaking

            - Replace a public field.

            ### Changed

            - Fix the parser.

        "}
    );
}

/// The mark belongs to one package of the fragment, so the other package's
/// section renders the same entry as a plain one.
#[test]
fn a_patch_bang_entry_is_breaking_only_in_its_package() {
    let [demo] = demo_plan();
    let core = PlanEntry {
        name: "demo-core".into(),
        ..demo.clone()
    };
    let cli = PlanEntry {
        name: "demo-cli".into(),
        ..demo
    };
    let fragment = Fragment {
        path: PathBuf::from("split.md"),
        packages: vec![
            PackageBump {
                name: "demo-core".into(),
                bump: Bump::Patch,
                breaking: true,
            },
            PackageBump {
                name: "demo-cli".into(),
                bump: Bump::Patch,
                breaking: false,
            },
        ],
        summary: "Split the config type".into(),
        group: None,
    };
    assert_eq!(
        release::changelog_sections(&[core, cli], &[fragment]).unwrap(),
        indoc! {"
            ## demo-core 1.0.1

            ### Breaking

            - Split the config type.

            ## demo-cli 1.0.1

            - Split the config type.

        "}
    );
}
