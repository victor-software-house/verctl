//! Lockfile parsing and comparison on real lockfiles: an excerpt of
//! ctl-core's `Cargo.lock` from its 0.7.0 Version PR, and the `bun.lock`,
//! `package-lock.json`, and `uv.lock` that bun, npm, and uv wrote for a
//! two-package workspace before and after `a` moved from 1.0.0 to 1.1.0.

use std::path::Path;

use verctl::lockfile::{Change, Format, Kind, Lockfile, kind};

fn read(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/lockfiles")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[expect(
    clippy::expect_used,
    reason = "a fixture that does not parse fails the test"
)]
fn parse(format: Format, text: &str) -> Lockfile {
    Lockfile::parse(format, text).expect("parse")
}

fn member(label: &str, name: &str, from: &str, to: &str) -> Change {
    Change::Member {
        label: label.to_owned(),
        name: name.to_owned(),
        from: Some(from.to_owned()),
        to: Some(to.to_owned()),
    }
}

#[test]
fn a_file_name_decides_how_check_reads_a_lockfile() {
    let cases = [
        ("Cargo.lock", Some(Kind::Parsed(Format::Cargo))),
        ("web/bun.lock", Some(Kind::Parsed(Format::Bun))),
        ("package-lock.json", Some(Kind::Parsed(Format::Npm))),
        ("npm-shrinkwrap.json", Some(Kind::Parsed(Format::Npm))),
        ("uv.lock", Some(Kind::Parsed(Format::Uv))),
        ("pnpm-lock.yaml", Some(Kind::Unchanged)),
        ("poetry.lock", Some(Kind::Unchanged)),
        ("pdm.lock", Some(Kind::Unchanged)),
        ("yarn.lock", Some(Kind::Unreadable)),
        ("bun.lockb", Some(Kind::Unreadable)),
        ("mise.lock", None),
        ("Cargo.toml", None),
    ];
    for (path, expected) in cases {
        assert_eq!(kind(Path::new(path)), expected, "{path}");
    }
}

#[test]
fn cargo_members_are_the_packages_without_a_source() {
    let lock = parse(Format::Cargo, &read("ctl-core.after.Cargo.lock"));
    let members: Vec<&str> = lock.members().map(|entry| entry.name.as_str()).collect();
    assert_eq!(members, ["ctl-core", "ctl-layout", "ctl-term", "ctl-text"]);
    let anstyle = lock
        .entries()
        .find(|entry| entry.name == "anstyle")
        .expect("anstyle");
    assert!(!anstyle.member);
    assert_eq!(anstyle.version.as_deref(), Some("1.0.14"));
}

#[test]
fn ctl_core_moves_four_members_and_nothing_else() {
    let before = parse(Format::Cargo, &read("ctl-core.before.Cargo.lock"));
    let after = parse(Format::Cargo, &read("ctl-core.after.Cargo.lock"));
    assert_eq!(
        before.compare(&after),
        [
            member("ctl-core", "ctl-core", "0.6.28", "0.7.0"),
            member("ctl-layout", "ctl-layout", "0.6.28", "0.7.0"),
            member("ctl-term", "ctl-term", "0.6.28", "0.7.0"),
            member("ctl-text", "ctl-text", "0.6.28", "0.7.0"),
        ]
    );
}

#[test]
fn a_registry_package_that_moves_is_a_change() {
    let before = read("ctl-core.before.Cargo.lock");
    let after = read("ctl-core.after.Cargo.lock").replace(
        "name = \"anstyle\"\nversion = \"1.0.14\"",
        "name = \"anstyle\"\nversion = \"1.0.15\"",
    );
    let changes = parse(Format::Cargo, &before).compare(&parse(Format::Cargo, &after));
    assert!(
        changes.contains(&Change::Changed("anstyle".to_owned())),
        "{changes:?}"
    );
    assert_eq!(
        changes
            .iter()
            .filter(|change| !change.is_member_version())
            .count(),
        1,
        "{changes:?}"
    );
}

#[test]
fn a_member_reference_with_a_version_follows_the_member() {
    let pin = |text: String, version: &str| {
        text.replace(
            " \"ctl-text\",\n \"insta\",",
            &format!(" \"ctl-text {version}\",\n \"insta\","),
        )
    };
    let before = pin(read("ctl-core.before.Cargo.lock"), "0.6.28");
    let after = pin(read("ctl-core.after.Cargo.lock"), "0.7.0");
    let changes = parse(Format::Cargo, &before).compare(&parse(Format::Cargo, &after));
    assert!(changes.iter().all(Change::is_member_version), "{changes:?}");
}

#[test]
fn an_added_package_and_a_changed_header_are_changes() {
    let before = read("ctl-core.before.Cargo.lock");
    let after = format!(
        "{}\n[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        before.replace("version = 4", "version = 3")
    );
    let changes = parse(Format::Cargo, &before).compare(&parse(Format::Cargo, &after));
    assert_eq!(changes, [Change::Header, Change::Added("serde".to_owned())]);
}

#[test]
fn bun_records_a_member_by_its_workspace_path() {
    let before = parse(Format::Bun, &read("workspace.before.bun.lock"));
    let after = parse(Format::Bun, &read("workspace.after.bun.lock"));
    assert_eq!(
        before.compare(&after),
        [member("packages/a", "a", "1.0.0", "1.1.0")]
    );
    let members: Vec<&str> = after.members().map(|entry| entry.label.as_str()).collect();
    assert_eq!(members, [".", "packages/a", "packages/b"]);
}

#[test]
fn a_bun_package_that_moves_is_a_change() {
    let before = read("workspace.before.bun.lock");
    let after = before.replace("is-number@7.0.0", "is-number@7.0.1");
    let changes = parse(Format::Bun, &before).compare(&parse(Format::Bun, &after));
    assert_eq!(changes, [Change::Changed("is-number".to_owned())]);
}

#[test]
fn npm_records_a_member_by_its_folder() {
    let before = parse(Format::Npm, &read("workspace.before.package-lock.json"));
    let after = parse(Format::Npm, &read("workspace.after.package-lock.json"));
    assert_eq!(
        before.compare(&after),
        [member("packages/a", "a", "1.0.0", "1.1.0")]
    );
    let members: Vec<&str> = after.members().map(|entry| entry.label.as_str()).collect();
    assert_eq!(members, [".", "packages/a", "packages/b"]);
}

#[test]
fn an_npm_package_that_moves_is_a_change() {
    let before = read("workspace.before.package-lock.json");
    let after = before.replace(
        "\"version\": \"7.0.0\",\n      \"resolved\"",
        "\"version\": \"7.0.1\",\n      \"resolved\"",
    );
    assert_ne!(before, after);
    let changes = parse(Format::Npm, &before).compare(&parse(Format::Npm, &after));
    assert_eq!(
        changes,
        [Change::Changed("node_modules/is-number".to_owned())]
    );
}

#[test]
fn uv_members_are_the_editable_and_virtual_packages() {
    let before = parse(Format::Uv, &read("workspace.before.uv.lock"));
    let after = parse(Format::Uv, &read("workspace.after.uv.lock"));
    assert_eq!(before.compare(&after), [member("a", "a", "1.0.0", "1.1.0")]);
    let members: Vec<&str> = after.members().map(|entry| entry.name.as_str()).collect();
    assert_eq!(members, ["a", "b", "demo-root"]);
}

#[test]
fn a_uv_package_that_moves_is_a_change() {
    let before = read("workspace.before.uv.lock");
    let after = before.replace(
        "name = \"six\"\nversion = \"1.16.0\"",
        "name = \"six\"\nversion = \"1.17.0\"",
    );
    let changes = parse(Format::Uv, &before).compare(&parse(Format::Uv, &after));
    assert_eq!(changes, [Change::Changed("six".to_owned())]);
}

#[test]
fn a_lockfile_that_does_not_parse_is_an_error() {
    assert!(Lockfile::parse(Format::Cargo, "[[package]\n").is_err());
    assert!(Lockfile::parse(Format::Bun, "").is_err());
    assert!(Lockfile::parse(Format::Npm, "{").is_err());
}
