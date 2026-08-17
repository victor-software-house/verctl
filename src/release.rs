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
use std::path::{Path, PathBuf};

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
        writedoc!(
            sections,
            "
            ## {name} {version}

            {bullets}
            "
        )?;
    }
    Ok(sections)
}

pub fn write_changelogs(
    config: &crate::config::Config,
    root: &Path,
    plan: &[PlanEntry],
    fragments: &[Fragment],
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for entry in plan {
        let spec = config.find(&entry.name)?;
        let path = spec.changelog_path(root);
        let section = changelog_section(entry, fragments)?;
        if section.is_empty() {
            continue;
        }
        prepend_changelog(&path, &section)?;
        written.push(path);
    }
    Ok(written)
}

fn changelog_section(entry: &PlanEntry, fragments: &[Fragment]) -> Result<String> {
    changelog_sections(std::slice::from_ref(entry), fragments)
}

/// First `## name version` or `## version` section from a changelog body.
#[must_use]
pub fn changelog_section_for(body: &str, name: &str, version: &str) -> Option<String> {
    let named = format!("## {name} {version}");
    let bare = format!("## {version}");
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        let heading = line.trim();
        if heading != named && heading != bare {
            continue;
        }
        let mut section = String::from(line);
        section.push('\n');
        for next in lines {
            if next.starts_with("## ") {
                break;
            }
            section.push_str(next);
            section.push('\n');
        }
        return Some(section);
    }
    None
}

/// GitHub Release body: the matching section from each package changelog.
#[must_use]
pub fn notes_for<'a>(
    config: &crate::config::Config,
    root: &Path,
    packages: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let mut sections = Vec::new();
    for (name, version) in packages {
        let Ok(spec) = config.find(name) else {
            continue;
        };
        let Ok(body) = fs::read_to_string(spec.changelog_path(root)) else {
            continue;
        };
        if let Some(section) = changelog_section_for(&body, name, version) {
            sections.push(section);
        }
    }
    sections.join("\n")
}

/// Fail unless every package version has a changelog heading.
pub fn require_changelog_versions<'a>(
    config: &crate::config::Config,
    root: &Path,
    packages: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<()> {
    let mut missing = Vec::new();
    for (name, version) in packages {
        let spec = config.find(name)?;
        let path = spec.changelog_path(root);
        let covered = fs::read_to_string(&path)
            .is_ok_and(|body| changelog_section_for(&body, name, version).is_some());
        if !covered {
            missing.push(format!("{name}@{version} ({path})", path = path.display()));
        }
    }
    ensure!(
        missing.is_empty(),
        "publish needs a changelog section for each version (the Version PR writes these): {}",
        missing.join(", ")
    );
    Ok(())
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
    changelog: &str,
) -> Result<Option<String>> {
    let repo = github::repo(root)?;
    match git::commit_branch_and_push(root, VERSION_BRANCH, message, token, &repo, paths)? {
        git::PushOutcome::Empty => return Ok(None),
        git::PushOutcome::Pushed => {}
    }
    let body = pr_body(changelog);
    if let Some(existing) = github::existing_pr(token, &repo, VERSION_BRANCH)? {
        return github::update_pr(token, &repo, existing.number, title, &body).map(Some);
    }
    let base = github::base_branch(root);
    github::create_pr(token, &repo, title, VERSION_BRANCH, &base, &body).map(Some)
}

#[must_use]
pub fn pr_body(changelog: &str) -> String {
    let body = changelog.trim();
    if body.is_empty() {
        "Prepared by verctl.".into()
    } else {
        body.to_owned()
    }
}
