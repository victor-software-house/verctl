use anyhow::{Context, Result};
use minijinja::{Environment, context};
use serde::Serialize;

const RELEASE_TEMPLATE: &str = include_str!("../templates/changelog.jinja");
const DEPENDENCY_TEMPLATE: &str = include_str!("../templates/dependency-changelog.jinja");

/// A pull request an entry links to.
#[derive(Debug, Clone, Serialize)]
pub struct PullRequest {
    /// The pull request number.
    pub number: u64,
    /// Link to the pull request.
    pub url: String,
    /// The author's login.
    pub user: Option<String>,
    /// Link to the author's profile.
    pub user_url: Option<String>,
    /// The author is outside `internal_authors`, so the entry credits them by
    /// name.
    pub external_author: bool,
}

/// A commit an entry links to when no pull request is known.
#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    /// Abbreviated commit SHA.
    pub short: String,
    /// Link to the commit.
    pub url: String,
}

/// A dependency whose version changed.
#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    /// The package name.
    pub name: String,
    /// The version the dependency was bumped to.
    pub new_version: String,
}

/// The data one fragment contributes to a release entry.
#[derive(Debug, Clone)]
pub struct ReleaseInput {
    /// The fragment's first line.
    pub summary: String,
    /// Fragment body lines after the summary, indented under the entry.
    pub continuations: Vec<String>,
    /// The pull request that added the fragment.
    pub pull_request: Option<PullRequest>,
    /// The commit that added the fragment, linked when no pull request is
    /// known.
    pub commit: Option<Commit>,
}

impl ReleaseInput {
    /// Marks the pull request as externally authored unless its author is in
    /// `internal_authors`.
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

/// Whether the summary already ends in `.`, `!`, or `?`.
#[must_use]
pub fn summary_has_terminal(summary: &str) -> bool {
    summary
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|ch| matches!(ch, '.' | '!' | '?'))
}

/// The summary a reference follows: one trailing period removed, so the entry
/// ends `… ([#12](url)).` with a single period, never `…. ([#12](url)).`. An
/// ellipsis (`...`) stays whole, since dropping one dot would leave `..`.
#[must_use]
pub fn summary_body(summary: &str) -> &str {
    let summary = summary.trim_end();
    if summary.ends_with("..") {
        return summary;
    }
    summary.strip_suffix('.').unwrap_or(summary)
}

/// Renders `input` with the stock release template.
pub fn render_release(input: &ReleaseInput) -> Result<String> {
    render_release_template(RELEASE_TEMPLATE, input)
}

/// Renders `input` with `template` in place of the stock release template.
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
            summary_body => summary_body(&input.summary),
            summary_has_terminal => summary_has_terminal(&input.summary),
        })
        .context("render changelog")?;
    Ok(rendered)
}

/// Renders the `Updated dependencies:` entry for `dependencies`.
pub fn render_dependencies(dependencies: &[Dependency]) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("deps", DEPENDENCY_TEMPLATE)
        .context("parse dependency template")?;
    let tmpl = env.get_template("deps")?;
    tmpl.render(context! { dependencies => dependencies })
        .context("render dependency changelog")
}
