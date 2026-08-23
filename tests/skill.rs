//! The bundled skill must describe this release, not a previous one.

use std::fs;
use std::path::Path;

fn skill() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/verctl/SKILL.md"))
        .unwrap_or_else(|error| panic!("read skills/verctl/SKILL.md: {error}"))
}

#[test]
fn skill_version_matches_the_package() {
    let skill = skill();
    let expected = format!("version: {}", env!("CARGO_PKG_VERSION"));
    assert!(
        skill.lines().any(|line| line == expected),
        "skill version must match Cargo.toml so the Version PR pin stays true:\n{skill}"
    );
}

#[test]
fn skill_documents_mise_run_ver_without_a_separator() {
    let skill = skill();
    assert!(
        skill.contains("mise run ver status"),
        "skill must show the operator form:\n{skill}"
    );
    assert!(
        !skill.contains("mise run ver -- "),
        "skill still tells operators to pass -- before verbs:\n{skill}"
    );
}
