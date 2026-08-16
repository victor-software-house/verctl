use indoc::indoc;
use std::fs;
use tempfile::TempDir;
use verctl::fragment::{Bump, load_dir, parse_str};

fn err_text(result: anyhow::Result<impl std::fmt::Debug>) -> String {
    format!("{:#}", result.expect_err("expected error"))
}

#[test]
fn parses_quoted_and_unquoted_names() {
    let fragment = parse_str(
        indoc! {r#"
            ---
            forkctl: patch
            "@scope/pkg": minor
            ---

            Restore mise.toml when later patches unapply.
        "#},
        "mixed.md",
    )
    .expect("parse");
    assert_eq!(fragment.packages.len(), 2);
    let forkctl = fragment
        .packages
        .iter()
        .find(|package| package.name == "forkctl")
        .expect("forkctl");
    let scoped = fragment
        .packages
        .iter()
        .find(|package| package.name == "@scope/pkg")
        .expect("scoped");
    assert_eq!(forkctl.bump, Bump::Patch);
    assert_eq!(scoped.bump, Bump::Minor);
    assert_eq!(fragment.max_bump(), Bump::Minor);
    assert_eq!(
        fragment.summary,
        "Restore mise.toml when later patches unapply."
    );
}

#[test]
fn accepts_none_and_major() {
    let fragment = parse_str(
        indoc! {"
            ---
            docs: none
            api: major
            ---

            Break the flag.
        "},
        "bumps.md",
    )
    .expect("parse");
    let docs = fragment
        .packages
        .iter()
        .find(|package| package.name == "docs")
        .expect("docs");
    let api = fragment
        .packages
        .iter()
        .find(|package| package.name == "api")
        .expect("api");
    assert_eq!(docs.bump, Bump::None);
    assert_eq!(api.bump, Bump::Major);
    assert_eq!(fragment.max_bump(), Bump::Major);
}

#[test]
fn rejects_unknown_bump() {
    let error = parse_str(
        indoc! {"
            ---
            forkctl: huge
            ---

            Nope.
        "},
        "bad.md",
    )
    .expect_err("unknown bump");
    assert!(format!("{error:#}").contains("unknown bump"), "{error:#}");
}

#[test]
fn rejects_missing_fence() {
    let error = parse_str("just a note\n", "nofence.md").expect_err("fence");
    assert!(
        format!("{error:#}").contains("must start with ---"),
        "{error:#}"
    );
}

#[test]
fn rejects_empty_mapping() {
    let error = parse_str(
        indoc! {"
            ---
            {}
            ---

            Empty.
        "},
        "empty.md",
    )
    .expect_err("empty");
    assert!(format!("{error:#}").contains("no packages"), "{error:#}");
}

#[test]
fn accepts_bom_and_crlf() {
    let raw = "\u{feff}---\r\nforkctl: patch\r\n---\r\n\r\nWindows file.\r\n";
    let fragment = parse_str(raw, "crlf.md").expect("parse");
    assert_eq!(fragment.packages[0].name, "forkctl");
    assert_eq!(fragment.summary, "Windows file.");
}

#[test]
fn rejects_numeric_bump() {
    let error = err_text(parse_str(
        indoc! {"
            ---
            forkctl: 1
            ---

            Number.
        "},
        "num.md",
    ));
    assert!(error.contains("must be a string"), "{error}");
}

#[test]
fn rejects_sequence_front_matter() {
    let error = err_text(parse_str(
        indoc! {"
            ---
            - forkctl
            ---

            List.
        "},
        "list.md",
    ));
    assert!(error.contains("must be a mapping"), "{error}");
}

#[test]
fn body_may_contain_horizontal_rule() {
    let fragment = parse_str(
        indoc! {"
            ---
            forkctl: patch
            ---

            First paragraph.

            ---

            Still the body.
        "},
        "rule.md",
    )
    .expect("parse");
    assert!(fragment.summary.contains("Still the body."));
}

#[test]
fn load_dir_skips_readme_and_missing_dir() {
    let root = TempDir::new().expect("tempdir");
    assert!(
        load_dir(&root.path().join("missing"))
            .expect("missing")
            .is_empty()
    );
    let dir = root.path().join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("README.md"), "not a fragment\n").expect("readme");
    fs::write(dir.join("config.json"), "{}\n").expect("config");
    fs::write(
        dir.join("ok.md"),
        indoc! {"
            ---
            forkctl: patch
            ---

            Ok.
        "},
    )
    .expect("ok");
    let loaded = load_dir(&dir).expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].packages[0].name, "forkctl");
}

#[test]
fn load_dir_fails_closed_on_one_bad_file() {
    let root = TempDir::new().expect("tempdir");
    let dir = root.path().join(".changeset");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("ok.md"),
        indoc! {"
            ---
            forkctl: patch
            ---

            Ok.
        "},
    )
    .expect("ok");
    fs::write(dir.join("bad.md"), "no fence\n").expect("bad");
    let error = err_text(load_dir(&dir).map(|_| ()));
    assert!(error.contains("must start with ---"), "{error}");
}
