use indoc::indoc;
use verctl::fragment::{Bump, parse_str};

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
    assert!(error.to_string().contains("unknown bump"));
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
    assert!(error.to_string().contains("no packages"));
}
