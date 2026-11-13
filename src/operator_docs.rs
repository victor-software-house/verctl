//! Operator files kept in the repository. Clap and the consumer's prose build
//! them.

use std::collections::BTreeMap;
use std::path::Path;
use std::{env, fs};

use ctl_core::surface;
use minijinja::{Value, context};

use crate::cli::Cli;
use crate::config::Config;
use crate::{git, release, templates};

const SKILL_TEMPLATE: &str = ".ctl/templates/SKILL.md.jinja";
const INSTRUCTIONS_TEMPLATE: &str = ".ctl/operator/instructions.md.jinja";
const SKILL: &str = "skills/verctl/SKILL.md";
const INSTRUCTIONS: &str = "src/instructions.md";

fn crate_file(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn render_template(relative: &str, context: &Value) -> String {
    let source = crate_file(relative);
    let environment =
        surface::environment().unwrap_or_else(|error| panic!("operator environment: {error}"));
    environment
        .template_from_named_str(relative, &source)
        .unwrap_or_else(|error| panic!("parse {relative}: {error}"))
        .render(context)
        .unwrap_or_else(|error| panic!("render {relative}: {error}"))
}

fn assert_committed(relative: &str, rendered: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    if env::var_os("UPDATE_OPERATOR_DOCS").is_some() {
        fs::write(&path, rendered)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    }
    assert_eq!(rendered, crate_file(relative));
}

fn surface() -> ctl_core::Surface {
    ctl_core::Surface::new::<Cli>("ver")
}

#[test]
fn skill_is_the_committed_surface_render() {
    let versions = BTreeMap::from([("verctl".to_owned(), env!("CARGO_PKG_VERSION").to_owned())]);
    let rendered = render_template(
        SKILL_TEMPLATE,
        &context! {
            verctl_surface => surface(),
            versions,
        },
    );
    assert_committed(SKILL, &rendered);
}

#[test]
fn instructions_are_the_committed_surface_render() {
    let rendered = render_template(
        INSTRUCTIONS_TEMPLATE,
        &context! {
            surface => surface(),
        },
    );
    assert_committed(INSTRUCTIONS, &rendered);
}

#[test]
fn every_served_template_renders() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        git::is_repository(root),
        "templates::plan discovers sources from the git index"
    );
    let config = Config::load(&root.join(".ctl/ver.yaml"))
        .unwrap_or_else(|error| panic!("load .ctl/ver.yaml: {error:#}"));
    let versions = release::served_versions(root, &config, &[]);
    let served = templates::plan(root, &config.templates, &config.pins, &versions)
        .unwrap_or_else(|error| panic!("{error:#}"));
    let skill = root.join(SKILL);
    assert!(
        served.iter().any(|path| path == &skill),
        "{SKILL_TEMPLATE} must still serve {SKILL}"
    );
}
