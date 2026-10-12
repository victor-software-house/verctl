use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{Context, Result, ensure};
use ctl_core::{formatdoc, writedoc};

use crate::changelog::{self, Commit, PullRequest, ReleaseInput};
use crate::config::Config;
use crate::fragment::Fragment;
use crate::prepare::PlanEntry;
use crate::{git, github};

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

/// Every package's version as the released tree will say it: what the manifests
/// say now, with this release's bumps applied over them.
///
/// A pin moves only what the release names, because everything else in that
/// file is already right. A template renders a whole file, so it needs every
/// version the file mentions — a served file that names a package no fragment
/// bumped would otherwise have nothing to render.
///
/// A manifest that cannot be read or resolved is left out rather than failing
/// the release: it is not this release's business unless something serves it,
/// and a template that names a missing version fails at render time, where the
/// complaint says which template and which name.
#[must_use]
pub fn served_versions(
    root: &Path,
    config: &Config,
    planned: &[(String, String)],
) -> Vec<(String, String)> {
    let mut versions: Vec<(String, String)> = config
        .packages
        .iter()
        .filter_map(|spec| {
            let raw = fs::read_to_string(root.join(&spec.path)).ok()?;
            let driver = spec.resolve(config, root).ok()?;
            let version = driver.read(&raw).ok()?;
            Some((spec.name.clone(), version.trim().to_owned()))
        })
        .collect();
    for (name, version) in planned {
        if let Some(entry) = versions.iter_mut().find(|(package, _)| package == name) {
            entry.1.clone_from(version);
        } else {
            versions.push((name.clone(), version.clone()));
        }
    }
    versions
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

/// Where each fragment came from: the pull request that merged it, or the
/// commit that added it. Empty unless `origin` is a GitHub remote: the links
/// name the repository the fragments were committed to, and a checkout
/// without that remote renders the same bare entries everywhere.
#[derive(Debug, Clone, Default)]
pub struct Origins {
    by_path: BTreeMap<PathBuf, Origin>,
}

#[derive(Debug, Clone)]
enum Origin {
    PullRequest(PullRequest),
    Commit(Commit),
}

/// Resolve each committed fragment to the commit that added it, and that
/// commit to the pull request its subject names (`… (#12)`, GitHub's squash
/// subject). No entry carries a byline: release notes credit no maintainer,
/// and telling one apart needs `changelog.internal_authors`, which waits for
/// 0.4.0 because a new `Config` field is a breaking library change.
pub fn origins(root: &Path, fragments: &[Fragment]) -> Result<Origins> {
    let Ok(repo) = git::origin_url(root).and_then(|url| github::parse_remote(&url)) else {
        return Ok(Origins::default());
    };
    let base = format!("https://github.com/{}/{}", repo.owner, repo.name);
    let mut by_path = BTreeMap::new();
    for fragment in fragments {
        let Some(added) = git::added_by(root, &fragment.path)? else {
            continue;
        };
        let origin = if let Some(number) = pull_request_number(&added.subject) {
            Origin::PullRequest(PullRequest {
                number,
                url: format!("{base}/pull/{number}"),
                user: None,
                user_url: None,
                external_author: false,
            })
        } else {
            Origin::Commit(Commit {
                short: added.sha.get(..7).unwrap_or(&added.sha).to_owned(),
                url: format!("{base}/commit/{}", added.sha),
            })
        };
        by_path.insert(fragment.path.clone(), origin);
    }
    Ok(Origins { by_path })
}

/// The pull request a squash-merge subject names: a trailing `(#12)`.
fn pull_request_number(subject: &str) -> Option<u64> {
    let (_, digits) = subject.trim_end().strip_suffix(')')?.rsplit_once("(#")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// [`changelog_sections_with`] with no references.
pub fn changelog_sections(plan: &[PlanEntry], fragments: &[Fragment]) -> Result<String> {
    changelog_sections_with(plan, fragments, &Origins::default())
}

/// One `## name version` section per plan entry, each fragment a bullet that
/// links where it came from when `origins` knows.
pub fn changelog_sections_with(
    plan: &[PlanEntry],
    fragments: &[Fragment],
    origins: &Origins,
) -> Result<String> {
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
            let origin = origins.by_path.get(&fragment.path);
            let line = changelog::render_release(&ReleaseInput {
                summary: fragment.summary.clone(),
                continuations: Vec::new(),
                pull_request: match origin {
                    Some(Origin::PullRequest(pull_request)) => Some(pull_request.clone()),
                    Some(Origin::Commit(_)) | None => None,
                },
                commit: match origin {
                    Some(Origin::Commit(commit)) => Some(commit.clone()),
                    Some(Origin::PullRequest(_)) | None => None,
                },
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

/// [`write_changelogs_with`] with no references.
pub fn write_changelogs(
    config: &crate::config::Config,
    root: &Path,
    plan: &[PlanEntry],
    fragments: &[Fragment],
) -> Result<Vec<PathBuf>> {
    write_changelogs_with(config, root, plan, fragments, &Origins::default())
}

/// Prepend each plan entry's section to its package CHANGELOG.md.
pub fn write_changelogs_with(
    config: &crate::config::Config,
    root: &Path,
    plan: &[PlanEntry],
    fragments: &[Fragment],
    origins: &Origins,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for entry in plan {
        let spec = config.find(&entry.name)?;
        let path = spec.changelog_path(root);
        let section = changelog_sections_with(std::slice::from_ref(entry), fragments, origins)?;
        if section.is_empty() {
            continue;
        }
        prepend_changelog(&path, &section)?;
        written.push(path);
    }
    Ok(written)
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

/// GitHub Release body: the matching changelog section for each package the
/// caller passes. Pass one package for a per-package Release; pass several
/// only when one Release covers them all.
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
    version_label: &str,
) -> Result<Option<String>> {
    let repo = github::repo(root)?;
    match git::commit_branch_and_push(root, VERSION_BRANCH, message, token, &repo, paths)? {
        git::PushOutcome::Empty => return Ok(None),
        git::PushOutcome::Pushed => {}
    }
    let body = pr_body(changelog);
    if let Some(existing) = github::existing_pr(token, &repo, VERSION_BRANCH)? {
        let url = github::update_pr(token, &repo, existing.number, title, &body)?;
        github::ensure_version_label(token, &repo, existing.number, version_label)?;
        return Ok(Some(url));
    }
    let base = github::base_branch(root);
    let created = github::create_pr(token, &repo, title, VERSION_BRANCH, &base, &body)?;
    github::ensure_version_label(token, &repo, created.number, version_label)?;
    Ok(Some(created.url))
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

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn a_squash_subject_names_its_pull_request() {
        assert_eq!(pull_request_number("fix: the parser (#12)"), Some(12));
        assert_eq!(pull_request_number("fix: the parser (#12)  "), Some(12));
        assert_eq!(pull_request_number("fix: the parser"), None);
        assert_eq!(pull_request_number("fix: (#12) in the middle"), None);
        assert_eq!(pull_request_number("fix: the parser (#)"), None);
        assert_eq!(pull_request_number("fix: the parser (#+12)"), None);
    }

    /// A release that bumps one of two packages still has to render a served
    /// file that mentions both, so the version map covers every package —
    /// the release's version for what it bumps, the manifest's for the rest.
    #[test]
    fn a_partial_release_still_knows_every_version() {
        let root = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("core")).unwrap();
        fs::write(
            root.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                name = "verctl"
                version = "0.0.3"
            "#},
        )
        .unwrap();
        fs::write(
            root.path().join("core/Cargo.toml"),
            indoc! {r#"
                [package]
                name = "ctl-core"
                version = "0.1.7"
            "#},
        )
        .unwrap();
        let config = Config::parse(indoc! {"
            packages:
              - name: verctl
                path: Cargo.toml
              - name: ctl-core
                path: core/Cargo.toml
        "})
        .unwrap();
        let planned = [("verctl".to_owned(), "0.0.4".to_owned())];
        assert_eq!(
            served_versions(root.path(), &config, &planned),
            [
                ("verctl".to_owned(), "0.0.4".to_owned()),
                ("ctl-core".to_owned(), "0.1.7".to_owned()),
            ]
        );
        fs::remove_file(root.path().join("core/Cargo.toml")).unwrap();
        assert_eq!(
            served_versions(root.path(), &config, &planned),
            [("verctl".to_owned(), "0.0.4".to_owned())],
            "a manifest nothing in this release can read is left out, not fatal"
        );
    }
}
