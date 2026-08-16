use anyhow::{Context, Result};
use minijinja::{Environment, context};
use serde::Serialize;

const RELEASE_TEMPLATE: &str = include_str!("../templates/changelog.jinja");
const DEPENDENCY_TEMPLATE: &str = include_str!("../templates/dependency-changelog.jinja");

#[derive(Debug, Clone, Serialize)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    pub user: Option<String>,
    pub user_url: Option<String>,
    pub external_author: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub short: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    pub name: String,
    pub new_version: String,
}

#[derive(Debug, Clone)]
pub struct ReleaseInput {
    pub summary: String,
    pub continuations: Vec<String>,
    pub pull_request: Option<PullRequest>,
    pub commit: Option<Commit>,
}

impl ReleaseInput {
    #[must_use]
    pub fn with_author_filter(mut self, internal_authors: &[String]) -> Self {
        if let Some(pr) = &mut self.pull_request {
            let login = pr.user.as_deref().unwrap_or("");
            pr.external_author =
                !login.is_empty() && !internal_authors.iter().any(|name| name == login);
        }
        self
    }
}

#[must_use]
pub fn summary_has_terminal(summary: &str) -> bool {
    summary
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|ch| matches!(ch, '.' | '!' | '?'))
}

pub fn render_release(input: &ReleaseInput) -> Result<String> {
    render_release_template(RELEASE_TEMPLATE, input)
}

pub fn render_release_template(template: &str, input: &ReleaseInput) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("changelog", template)
        .context("parse changelog template")?;
    let tmpl = env.get_template("changelog")?;
    let rendered = tmpl
        .render(context! {
            summary => input.summary,
            continuations => input.continuations,
            pull_request => input.pull_request,
            commit => input.commit,
            summary_has_terminal => summary_has_terminal(&input.summary),
        })
        .context("render changelog")?;
    Ok(rendered)
}

pub fn render_dependencies(dependencies: &[Dependency]) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("deps", DEPENDENCY_TEMPLATE)
        .context("parse dependency template")?;
    let tmpl = env.get_template("deps")?;
    tmpl.render(context! { dependencies => dependencies })
        .context("render dependency changelog")
}
