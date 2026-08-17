use crate::changelog::{self, ReleaseInput};
use crate::fragment::Fragment;
use crate::git;
use crate::github;
use crate::prepare::PlanEntry;
use anyhow::{Context, Result, ensure};
use ctl_core::{formatdoc, writedoc};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub const VERSION_BRANCH: &str = "version-packages";

/// Auth for `--pr`.
///
/// The token is the one the environment already has (`GITHUB_TOKEN` in
/// Actions). We do not call `gh` and we do not use the logged-in
/// account. Local `--pr` is recovery when that same token is in the env.
pub fn resolve_token() -> Result<String> {
    env_token().context(
        "prepare --pr needs GITHUB_TOKEN (Actions sets it; locally export the same token — not `gh`)",
    )
}

fn env_token() -> Option<String> {
    for key in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(value) = env::var(key)
            && !value.is_empty()
        {
            return Some(value);
        }
    }
    None
}

/// Fragments that named at least one package in `plan`.
#[must_use]
pub fn contributing_fragments<'a>(
    plan: &[PlanEntry],
    fragments: &'a [Fragment],
) -> Vec<&'a Fragment> {
    fragments
        .iter()
        .filter(|fragment| {
            fragment
                .packages
                .iter()
                .any(|package| plan.iter().any(|entry| entry.name == package.name))
        })
        .collect()
}

pub fn consume_fragments<'a, I>(fragments: I) -> Result<()>
where
    I: IntoIterator<Item = &'a Fragment>,
{
    for fragment in fragments {
        fs::remove_file(&fragment.path)
            .with_context(|| format!("delete {}", fragment.path.display()))?;
    }
    Ok(())
}

pub fn changelog_sections(plan: &[PlanEntry], fragments: &[Fragment]) -> Result<String> {
    let mut sections = String::new();
    for entry in plan {
        let mut bullets = String::new();
        for fragment in fragments {
            if !fragment
                .packages
                .iter()
                .any(|package| package.name == entry.name)
            {
                continue;
            }
            let line = changelog::render_release(&ReleaseInput {
                summary: fragment.summary.clone(),
                continuations: Vec::new(),
                pull_request: None,
                commit: None,
            })?;
            bullets.push_str(&line);
            if !line.ends_with('\n') {
                bullets.push('\n');
            }
        }
        ensure!(
            !bullets.is_empty(),
            "no changelog body for package {}",
            entry.name
        );
        let name = entry.name.as_str();
        let version = entry.to.as_str();
        let _ = writedoc!(
            sections,
            "
            ## {name} {version}

            {bullets}
            "
        );
    }
    Ok(sections)
}

pub fn write_changelogs(
    changelog: &Path,
    plan: &[PlanEntry],
    fragments: &[Fragment],
) -> Result<()> {
    prepend_changelog(changelog, &changelog_sections(plan, fragments)?)
}

pub fn prepend_changelog(path: &Path, section: &str) -> Result<()> {
    match fs::read_to_string(path) {
        Ok(existing) => {
            let next = if let Some(rest) = existing.strip_prefix("# Changelog\n") {
                let rest = rest.trim_start();
                formatdoc! {"
                    # Changelog

                    {section}{rest}"
                }
            } else {
                formatdoc!("{section}{existing}")
            };
            fs::write(path, next).with_context(|| path.display().to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::write(
            path,
            formatdoc! {"
                    # Changelog

                    {section}"
            },
        )
        .with_context(|| path.display().to_string()),
        Err(error) => Err(error).with_context(|| path.display().to_string()),
    }
}

pub fn open_or_update_pr(
    root: &Path,
    token: &str,
    title: &str,
    message: &str,
    paths: &[std::path::PathBuf],
) -> Result<Option<String>> {
    let repo = github::repo(root)?;
    match git::commit_branch_and_push(root, VERSION_BRANCH, message, token, &repo, paths)? {
        git::PushOutcome::Empty => return Ok(None),
        git::PushOutcome::Pushed => {}
    }
    if let Some(url) = github::existing_pr(token, &repo, VERSION_BRANCH)? {
        return Ok(Some(url));
    }
    let base = github::base_branch(root);
    github::create_pr(
        token,
        &repo,
        title,
        VERSION_BRANCH,
        &base,
        "Prepared by verctl.",
    )
    .map(Some)
}
