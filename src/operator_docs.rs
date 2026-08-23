//! Clap verbs the operator skill never names. Not a list of required sentences.

use crate::cli::Cli;
use clap::CommandFactory;
use std::fs;
use std::path::Path;

const SKILL_TEMPLATE: &str = ".ctl/templates/SKILL.md.jinja";

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
