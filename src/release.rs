use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{Context, Result, ensure};
use ctl_core::{formatdoc, writedoc};

use crate::changelog::{self, Commit, PullRequest, ReleaseInput};
use crate::config::Config;
use crate::fragment::{Bump, Fragment, Group};
use crate::git::pull_request_number;
use crate::prepare::PlanEntry;
use crate::{git, github};

/// What every Version PR branch starts with. The rest is the base branch, so
/// each base gets its own Version PR and the name shows which one it is.
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

/// Auth for the commands that call the GitHub API.
///
/// The token is the one the environment already has (`GITHUB_TOKEN` in
/// Actions). We do not call `gh` and we do not use the logged-in
/// account. A local run needs that same token in the env.
///
/// # Errors
///
/// Returns an error when neither `GITHUB_TOKEN` nor `GH_TOKEN` is set to a
/// non-empty value.
pub fn resolve_token() -> Result<String> {
    env_token().context(
        "this command needs GITHUB_TOKEN or GH_TOKEN (Actions sets GITHUB_TOKEN; locally export the same token, since verctl does not read `gh` logins)",
    )
}

/// `GITHUB_TOKEN`, else `GH_TOKEN`, when set to a non-empty value.
#[must_use]
pub fn env_token() -> Option<String> {
    for key in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(value) = env::var(key)
            && !value.is_empty()
        {
            return Some(value);
        }
    }
    None
}

/// Every package's version in the released tree.
/// Each version is the one in the manifests now, with this release's bumps
/// applied over it.
///
/// A pin moves only what the release names, because everything else in that
/// file is already right.
/// Yet a template renders a whole file, so it needs every version the file
/// mentions.
/// Without this map, a served file that mentions a package no fragment bumped
/// would have nothing to render.
///
/// The map leaves out a manifest that cannot be read or resolved, instead of
/// failing the release.
/// That manifest matters to the release only when something serves it.
/// A template that mentions a missing version fails at render time, and the
/// error names the template and the package.
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
            let current = spec.current(config, root).ok()?;
            Some((spec.name.clone(), current.version))
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

/// Fragments the version step consumes.
/// Those are each fragment that names a package in `plan`, and each fragment
/// that bumps nothing, such as one holding only `demo: none`.
/// No later Version PR would consume that second kind either.
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
                .all(|package| package.bump == Bump::None)
                || fragment
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
/// commit that added it.
/// Empty unless `origin` is a GitHub remote.
/// The links point to the repository the fragments were committed to, and a
/// checkout without that remote renders the same bare entries everywhere.
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
/// commit to the pull request its subject names (`… (#N)`, GitHub's squash
/// subject).
/// With a token, the pull request author is credited when [`credits`] allows
/// it.
/// A failed lookup credits no one, so a maintainer is never named by
/// accident.
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
/// links where it came from when `origins` holds the origin.
///
/// A fragment's first paragraph is the bullet, so the link and the period
/// follow its last line.
/// Every later line is indented under the bullet.
/// A second paragraph, a nested list, or a code fence therefore stays inside
/// the entry ([CommonMark list items]).
/// The GitHub Release body is the same section.
///
/// [CommonMark list items]: https://spec.commonmark.org/0.31.2/#list-items
///
/// # Errors
///
/// Returns an error when a fragment's entry cannot be rendered.
/// It also returns one when no fragment names the package of a plan entry,
/// which leaves that section without a body.
pub fn changelog_sections_with(
    plan: &[PlanEntry],
    fragments: &[Fragment],
    origins: &Origins,
) -> Result<String> {
    let mut sections = String::new();
    for entry in plan {
        let mut entries: Vec<Entry> = Vec::new();
        for fragment in fragments {
            let Some(named) = fragment
                .packages
                .iter()
                .find(|package| package.name == entry.name)
            else {
                continue;
            };
            let origin = origins.by_path.get(&fragment.path);
            let lines: Vec<&str> = fragment.summary.lines().collect();
            let paragraph_end = lines
                .iter()
                .position(|line| line.trim().is_empty())
                .unwrap_or(lines.len());
            let (paragraph, rest) = lines.split_at(paragraph_end);
            let line = changelog::render_release(&ReleaseInput {
                summary: paragraph.join("\n"),
                continuations: rest.iter().map(|line| (*line).to_owned()).collect(),
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
            entries.push(Entry {
                group: fragment.group,
                breaking: named.breaking,
                line,
            });
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

/// One fragment's rendered entry in one package's section.
struct Entry {
    /// The fragment's `verctl:group`.
    group: Option<Group>,
    /// Whether the fragment bumps this section's package by `patch!`.
    breaking: bool,
    /// The rendered bullet.
    line: String,
}

/// The heading over a `patch!` entry. It comes first and is not a [`Group`],
/// so only the mark puts an entry under it.
const BREAKING: &str = "Breaking";

/// Join one section's entries. A section where no fragment names a group and
/// none is `patch!` stays the flat list it always was, so existing changelogs
/// render unchanged. Otherwise a `patch!` entry goes under [`BREAKING`], first,
/// and every other entry under a [`Group`] heading, an ungrouped one under
/// [`Group::Changed`].
fn group_entries(entries: &[Entry]) -> String {
    if entries
        .iter()
        .all(|entry| entry.group.is_none() && !entry.breaking)
    {
        return entries.iter().map(|entry| entry.line.as_str()).collect();
    }
    let breaking = (
        BREAKING,
        entries
            .iter()
            .filter(|entry| entry.breaking)
            .collect::<Vec<_>>(),
    );
    let grouped = Group::ALL.into_iter().map(|heading| {
        (
            heading.heading(),
            entries
                .iter()
                .filter(|entry| !entry.breaking && entry.group.unwrap_or(Group::Changed) == heading)
                .collect::<Vec<_>>(),
        )
    });
    std::iter::once(breaking)
        .chain(grouped)
        .filter(|(_, members)| !members.is_empty())
        .map(|(heading, members)| {
            let lines: String = members.iter().map(|entry| entry.line.as_str()).collect();
            format!("### {heading}\n\n{lines}")
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
/// Returns an error when:
///
/// - A plan entry names a package missing from the config.
/// - Its section cannot be built (see [`changelog_sections_with`]).
/// - Its changelog cannot be read or written.
///
/// Changelogs before the failing one are already written.
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
/// Returns an error when a package is missing from the config.
/// It also returns one when a package's changelog is unreadable or has no
/// `## name version` or `## version` heading for it.
/// The latter error lists every such package.
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
        "release needs a changelog section for each version (the Version PR writes these): {}",
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
/// Resolved before `version` writes anything, so a run that cannot resolve its
/// base fails and leaves the tree as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPr {
    /// `proposal.title`; unset or empty means
    /// [`Proposal::DEFAULT_TITLE`](crate::config::Proposal::DEFAULT_TITLE).
    pub title: String,
    /// `proposal.commit_message`; unset or empty means the title.
    pub message: String,
    /// The branch resolved by [`github::base_branch`].
    pub base: String,
}

impl VersionPr {
    /// Take the title and message from `proposal` and resolve the base.
    ///
    /// # Errors
    ///
    /// Returns an error when no base branch can be resolved, as
    /// [`github::base_branch`] describes.
    pub fn new(proposal: &crate::config::Proposal, root: &Path) -> Result<Self> {
        Ok(Self {
            title: proposal.title().to_owned(),
            message: proposal.commit_message().to_owned(),
            base: github::base_branch(root)?,
        })
    }
}

/// The Version PR `open_or_update_pr` opened or updated.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OpenedPr {
    /// The PR number.
    pub number: u64,
    /// The PR's web URL.
    pub url: String,
}

/// Push the Version PR branch and open the PR, or update the open one. `None`
/// when nothing changed.
///
/// # Errors
///
/// Returns an error when:
///
/// - The GitHub repository cannot be resolved.
/// - The branch cannot be committed or pushed (see
///   [`git::commit_branch_and_push`]).
/// - Looking up, updating, or creating the pull request fails.
///
/// A failure after the push leaves the branch pushed.
pub fn open_or_update_pr(
    root: &Path,
    token: &str,
    pr: &VersionPr,
    paths: &[std::path::PathBuf],
    changelog: &str,
) -> Result<Option<OpenedPr>> {
    let repo = github::repo(root)?;
    let branch = version_branch(&pr.base);
    match git::commit_branch_and_push(root, &branch, &pr.message, token, &repo, paths)? {
        git::PushOutcome::Empty => return Ok(None),
        git::PushOutcome::Pushed => {}
    }
    let body = pr_body(changelog);
    if let Some(existing) = github::existing_pr(token, &repo, &branch)? {
        let url = github::update_pr(token, &repo, existing.number, &pr.title, &body)?;
        return Ok(Some(OpenedPr {
            number: existing.number,
            url,
        }));
    }
    let created = github::create_pr(token, &repo, &pr.title, &branch, &pr.base, &body)?;
    Ok(Some(OpenedPr {
        number: created.number,
        url: created.url,
    }))
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
    /// file that mentions both.
    /// The version map therefore covers every package, with the release's
    /// version for what it bumps and the manifest's version for the rest.
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
