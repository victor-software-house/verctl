//! Clap verbs the operator skill never names. Not a list of required sentences.

use crate::cli::Cli;
use clap::CommandFactory;
use gray_matter::Matter;
use gray_matter::engine::YAML;
use serde::Deserialize;
use std::fs;
use std::path::Path;

const SKILL_TEMPLATE: &str = ".ctl/templates/SKILL.md.jinja";
const SKILL: &str = "skills/verctl/SKILL.md";

fn crate_file(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn verbs() -> Vec<String> {
    Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect()
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|token| !token.is_empty())
}

fn unnamed<'a>(body: &'a str, verb: &'a str) -> bool {
    !tokens(body).any(|token| token == verb)
}

#[test]
fn skill_template_pins_the_package_version() {
    let template = crate_file(SKILL_TEMPLATE);
    assert!(
        template
            .lines()
            .any(|line| line.trim() == r#"version: {{ versions["verctl"] }}"#),
        "{SKILL_TEMPLATE} must take its version from the Version PR context"
    );
}

#[derive(Debug, Deserialize)]
struct SkillFront {
    version: String,
}

#[test]
fn served_skill_version_matches_the_package() {
    let raw = crate_file(SKILL);
    let mut matter = Matter::<YAML>::new();
    matter.excerpt_delimiter = Some("\u{0000}".into());
    let parsed = matter
        .parse::<SkillFront>(&raw)
        .unwrap_or_else(|error| panic!("skill front matter: {error:#}"));
    let front = parsed
        .data
        .unwrap_or_else(|| panic!("{SKILL} must start with ---"));
    assert_eq!(front.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn every_served_template_renders() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = crate::config::Config::load(&root.join(".ctl/ver.yaml"))
        .unwrap_or_else(|error| panic!("load .ctl/ver.yaml: {error:#}"));
    let served = crate::templates::plan(
        root,
        &config.templates,
        &[(
            env!("CARGO_PKG_NAME").to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        )],
    )
    .unwrap_or_else(|error| panic!("{error:#}"));
    let skill = root.join(SKILL);
    assert!(
        served.iter().any(|path| path == &skill),
        "{SKILL_TEMPLATE} must still serve {SKILL}"
    );
}

#[test]
fn skill_template_names_every_clap_verb() {
    let body = crate_file(SKILL_TEMPLATE);
    let missing: Vec<String> = verbs()
        .into_iter()
        .filter(|verb| unnamed(&body, verb))
        .collect();
    assert!(
        missing.is_empty(),
        "{SKILL_TEMPLATE} never names {missing:?} (any mention counts)"
    );
}
