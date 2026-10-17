use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{Context, Result, ensure};
use ctl_core::{formatdoc, writedoc};

use crate::changelog::{self, Commit, PullRequest, ReleaseInput};
use crate::config::Config;
use crate::fragment::{Fragment, Group};
use crate::git::pull_request_number;
use crate::prepare::PlanEntry;
use crate::{git, github};

/// What every Version PR branch starts with. The rest is the base branch, so
/// each base gets its own Version PR and the name says which one it is.
pub const VERSION_BRANCH_PREFIX: &str = "verctl/release/";

/// The branch the Version PR into `base` is pushed to:
/// `verctl/release/<base>`. It is derived, never configured, because the name
/// is how verctl and its readers recognise a Version PR.
#[must_use]
pub fn version_branch(base: &str) -> String {
    format!("{VERSION_BRANCH_PREFIX}{base}")
}

/// Whether `branch` is a Version PR branch: [`VERSION_BRANCH_PREFIX`]
/// followed by a base branch name.
#[must_use]
pub fn is_version_branch(branch: &str) -> bool {
    branch
        .strip_prefix(VERSION_BRANCH_PREFIX)
        .is_some_and(|base| !base.is_empty())
}

/// Auth for `--pr`.
///
/// The token is the one the environment already has (`GITHUB_TOKEN` in
/// Actions). We do not call `gh` and we do not use the logged-in
/// account. Local `--pr` is recovery when that same token is in the env.
///
/// # Errors
///
/// Returns an error when neither `GITHUB_TOKEN` nor `GH_TOKEN` is set to a
/// non-empty value.
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

/// Delete the fragment files once a Version PR has consumed them.
///
/// # Errors
///
/// Returns an error naming the first fragment file that cannot be deleted.
/// Fragments before it are already gone.
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
/// subject). With a token, the pull request author is credited when
/// [`credits`] says so; a failed lookup credits no one, so a maintainer is
/// never named by accident.
///
/// # Errors
///
/// Returns an error when the history walk that finds the commit adding a
/// fragment fails, as [`git::added_by`] describes. A missing GitHub `origin`
/// is not an error and yields empty origins.
pub fn origins(
    root: &Path,
    token: Option<&str>,
    changelog: &crate::config::Changelog,
    fragments: &[Fragment],
) -> Result<Origins> {
    let Ok(repo) = git::origin_url(root).and_then(|url| github::parse_remote(&url)) else {
        return Ok(Origins::default());
    };
    let base = format!("https://github.com/{}/{}", repo.owner, repo.name);
    let mut authors: BTreeMap<u64, Option<String>> = BTreeMap::new();
    let mut by_path = BTreeMap::new();
    for fragment in fragments {
        let Some(added) = git::added_by(root, &fragment.path)? else {
            continue;
        };
        let origin = if let Some(number) = pull_request_number(&added.subject) {
            let credited = authors
                .entry(number)
                .or_insert_with(|| {
                    token
                        .and_then(|token| github::pull_author(token, &repo, number).ok())
                        .filter(|author| credits(changelog, author))
                        .map(|author| author.login)
                })
                .clone();
            Origin::PullRequest(PullRequest {
                number,
                url: format!("{base}/pull/{number}"),
                user_url: credited
                    .as_ref()
                    .map(|login| format!("https://github.com/{login}")),
                external_author: credited.is_some(),
                user: credited,
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

/// Release notes credit no maintainer: a byline needs a login outside
/// `changelog.internal_authors`, a person rather than a bot, and a pull request
/// from a fork. The fork test only narrows the list; it never credits a listed
/// login.
#[must_use]
pub fn credits(changelog: &crate::config::Changelog, author: &github::PullAuthor) -> bool {
    author.kind == "User" && author.from_fork && !changelog.is_internal(&author.login)
}

/// [`changelog_sections_with`] with no references.
///
/// # Errors
///
/// Returns an error under the conditions of [`changelog_sections_with`].
pub fn changelog_sections(plan: &[PlanEntry], fragments: &[Fragment]) -> Result<String> {
    changelog_sections_with(plan, fragments, &Origins::default())
}

/// One `## name version` section per plan entry, each fragment a bullet that
/// links where it came from when `origins` knows.
///
/// # Errors
///
/// Returns an error when a fragment's entry cannot be rendered, or when no
/// fragment names a plan entry's package, leaving that section without a body.
pub fn changelog_sections_with(
    plan: &[PlanEntry],
    fragments: &[Fragment],
    origins: &Origins,
) -> Result<String> {
    let mut sections = String::new();
    for entry in plan {
        let mut entries: Vec<(Option<Group>, String)> = Vec::new();
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
            let mut line = line;
            if !line.ends_with('\n') {
                line.push('\n');
            }
            entries.push((fragment.group, line));
        }
        let bullets = group_entries(&entries);
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

/// Join one section's entries. A section where no fragment names a group stays
/// the flat list it always was, so existing changelogs render unchanged. Once
/// one does, every entry goes under a [`Group`] heading, and an ungrouped one
/// lands under [`Group::Changed`].
fn group_entries(entries: &[(Option<Group>, String)]) -> String {
    if entries.iter().all(|(group, _)| group.is_none()) {
        return entries.iter().map(|(_, line)| line.as_str()).collect();
    }
    Group::ALL
        .into_iter()
        .filter_map(|heading| {
            let lines: String = entries
                .iter()
                .filter(|(group, _)| group.unwrap_or(Group::Changed) == heading)
                .map(|(_, line)| line.as_str())
                .collect();
            (!lines.is_empty()).then(|| format!("### {}\n\n{lines}", heading.heading()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`write_changelogs_with`] with no references.
///
/// # Errors
///
/// Returns an error under the conditions of [`write_changelogs_with`].
pub fn write_changelogs(
    config: &crate::config::Config,
    root: &Path,
    plan: &[PlanEntry],
    fragments: &[Fragment],
) -> Result<Vec<PathBuf>> {
    write_changelogs_with(config, root, plan, fragments, &Origins::default())
}

/// Prepend each plan entry's section to its package CHANGELOG.md.
///
/// Returns the changelog paths written.
///
/// # Errors
///
/// Returns an error when a plan entry names a package missing from the
/// config, when its section cannot be built (see
/// [`changelog_sections_with`]), or when its changelog cannot be read or
/// written. Changelogs before the failing one are already written.
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
///
/// # Errors
///
/// Returns an error when a package is missing from the config, or when any
/// package's changelog is unreadable or has no `## name version` or
/// `## version` heading for it. The latter error lists every such package.
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

/// Add a changelog section above the existing entries, keeping the title.
///
/// A missing file is created with a `# Changelog` title.
///
/// # Errors
///
/// Returns an error, naming `path`, when the file exists but cannot be read,
/// or when it cannot be written.
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

/// What the Version PR is called and which branch it targets.
///
/// Resolved before `prepare` writes anything, so a run that cannot tell its
/// base fails with the tree untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPr {
    /// `VERCTL_PR_TITLE`; unset or empty means `chore(release): version
    /// packages`.
    pub title: String,
    /// `VERCTL_COMMIT_MESSAGE`; unset or empty means the title.
    pub message: String,
    /// The branch [`github::base_branch`] names.
    pub base: String,
}

impl VersionPr {
    /// Read the title and message from the environment and resolve the base.
    ///
    /// An empty variable counts as unset: a composite action passes an input
    /// it was not given as `""`.
    ///
    /// # Errors
    ///
    /// Returns an error when no base branch can be resolved, as
    /// [`github::base_branch`] describes.
    pub fn from_env(root: &Path) -> Result<Self> {
        let title = non_empty_env("VERCTL_PR_TITLE")
            .unwrap_or_else(|| "chore(release): version packages".into());
        let message = non_empty_env("VERCTL_COMMIT_MESSAGE").unwrap_or_else(|| title.clone());
        Ok(Self {
            title,
            message,
            base: github::base_branch(root)?,
        })
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

/// Push the Version PR branch and open the PR, or update the open one. `None`
/// when nothing changed.
///
/// # Errors
///
/// Returns an error when the GitHub repository cannot be resolved, when the
/// branch cannot be committed or pushed (see [`git::commit_branch_and_push`]),
/// or when looking up, updating, or creating the pull request, or applying
/// `version_label`, fails. A failure after the push leaves the branch pushed.
pub fn open_or_update_pr(
    root: &Path,
    token: &str,
    pr: &VersionPr,
    paths: &[std::path::PathBuf],
    changelog: &str,
    version_label: &str,
) -> Result<Option<String>> {
    let repo = github::repo(root)?;
    let branch = version_branch(&pr.base);
    match git::commit_branch_and_push(root, &branch, &pr.message, token, &repo, paths)? {
        git::PushOutcome::Empty => return Ok(None),
        git::PushOutcome::Pushed => {}
    }
    let body = pr_body(changelog);
    if let Some(existing) = github::existing_pr(token, &repo, &branch)? {
        let url = github::update_pr(token, &repo, existing.number, &pr.title, &body)?;
        github::ensure_version_label(token, &repo, existing.number, version_label)?;
        return Ok(Some(url));
    }
    let created = github::create_pr(token, &repo, &pr.title, &branch, &pr.base, &body)?;
    github::ensure_version_label(token, &repo, created.number, version_label)?;
    Ok(Some(created.url))
}

/// Refuse auto-merge before `prepare --pr` writes anything.
///
/// The token must not be the job's own, which the workflow names in
/// `VERCTL_JOB_TOKEN`: a merge made with `GITHUB_TOKEN` starts no workflow, so
/// the release after it would never run. And a ruleset on `base` must require
/// a status check, or GitHub merges the moment auto-merge is on.
pub fn require_auto_merge(root: &Path, token: &str, base: &str) -> Result<()> {
    ensure!(
        non_empty_env("VERCTL_JOB_TOKEN").as_deref() != Some(token),
        "auto-merge needs a token other than the job's GITHUB_TOKEN, such as a GitHub App token: a merge made with GITHUB_TOKEN starts no workflow, so the release would not run"
    );
    let repo = github::repo(root)?;
    ensure!(
        github::requires_status_check(token, &repo, base)?,
        "{base} has no required status check; auto-merge would merge the Version PR at once"
    );
    Ok(())
}

/// Enable auto-merge on the open Version PR into `base`, and return its
/// number; `None` when no Version PR is open.
pub fn auto_merge(root: &Path, token: &str, base: &str) -> Result<Option<u64>> {
    let repo = github::repo(root)?;
    let Some(pr) = github::existing_pr(token, &repo, &version_branch(base))? else {
        return Ok(None);
    };
    github::enable_auto_merge(token, &repo, pr.number)?;
    Ok(Some(pr.number))
}

/// The PR body: the changelog, trimmed, or a stock line when it is empty.
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

    fn author(login: &str, kind: &str, from_fork: bool) -> github::PullAuthor {
        github::PullAuthor {
            login: login.into(),
            kind: kind.into(),
            from_fork,
        }
    }

    fn maintainers() -> crate::config::Changelog {
        crate::config::Changelog {
            internal_authors: vec!["any-victor".into()],
        }
    }

    #[test]
    fn a_listed_maintainer_on_an_in_repo_branch_is_not_credited() {
        assert!(!credits(
            &maintainers(),
            &author("any-victor", "User", false)
        ));
    }

    #[test]
    fn a_listed_maintainer_from_a_fork_is_not_credited() {
        assert!(!credits(
            &maintainers(),
            &author("any-victor", "User", true)
        ));
        assert!(!credits(
            &maintainers(),
            &author("Any-Victor", "User", true)
        ));
    }

    #[test]
    fn an_outside_contributor_from_a_fork_is_credited() {
        assert!(credits(&maintainers(), &author("octo", "User", true)));
    }

    #[test]
    fn a_bot_is_not_credited() {
        assert!(!credits(
            &maintainers(),
            &author("renovate[bot]", "Bot", true)
        ));
    }

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
