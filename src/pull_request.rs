//! What `check versions` requires of a pull request that is not the Version
//! PR, beyond its versions.
//!
//! Such a pull request may not add a CHANGELOG section for a version. Only the
//! Version PR writes one, for the version it moves to.
//!
//! A pull request that changes whether a package publishes must add a fragment
//! that bumps it. The next Version PR then releases that package under the new
//! setting.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use semver::Version;

use crate::config::{self, Config, PackageSpec};
use crate::content::{self, Inputs, Scratch};
use crate::fragment::{self, Bump};
use crate::git::{self, MergeBase};
use crate::publisher::{self, Publisher};

/// Each change against `base` that only the Version PR may make. One is a
/// CHANGELOG section for a version. Another is a package's publish setting
/// changed without an added fragment that bumps it. [`crate::versions::report`]
/// fails with them unless the pull request is the Version PR.
///
/// `base` is the merge-base with the pull request's base.
/// `release_candidates` name the release branch. A section or a publish
/// setting that HEAD's merge-base with the release branch already holds came
/// with a merge of that branch. It does not count.
///
/// The publish-setting rule needs the config at the merge-base.
/// That config may not load, as when this pull request removes a key the
/// running verctl refuses.
/// The rule is then skipped, and a notice names it and the load error.
///
/// # Errors
///
/// Returns an error when any of these cannot be read:
///
/// - the release branch's merge-base, or a CHANGELOG;
/// - a manifest there or in the working tree, which must also parse.
pub fn problems(
    root: &Path,
    config: &Config,
    inputs: &Inputs,
    base: &MergeBase,
    release_candidates: &[String],
) -> Result<Findings> {
    let mut found = Findings {
        problems: added_sections(root, config, base, release_candidates)?,
        notices: Vec::new(),
    };
    match publish_changes(root, config, inputs, base, release_candidates) {
        Ok(changes) => found.problems.extend(changes),
        Err(error) if error.downcast_ref::<content::BaseConfigFailed>().is_some() => {
            found
                .notices
                .push(format!("the publish-setting rule is skipped: {error:#}"));
        }
        Err(error) => return Err(error),
    }
    Ok(found)
}

/// What the pull request rules found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Findings {
    /// Each change only the Version PR may make.
    pub problems: Vec<String>,
    /// Each rule skipped, and why.
    pub notices: Vec<String>,
}

/// Each version section the working tree adds to a package CHANGELOG.
fn added_sections(
    root: &Path,
    config: &Config,
    base: &MergeBase,
    release_candidates: &[String],
) -> Result<Vec<String>> {
    let root = root.canonicalize().context("the config root")?;
    let paths: BTreeSet<PathBuf> = config
        .packages
        .iter()
        .map(|spec| spec.changelog_path(&root))
        .collect();
    let rels = paths
        .iter()
        .map(|path| {
            path.strip_prefix(&root)
                .with_context(|| format!("{} is outside {}", path.display(), root.display()))
        })
        .collect::<Result<Vec<&Path>>>()?;
    let released = git::files_on_merge_base(&root, &rels, release_candidates)?;
    let mut problems = Vec::new();
    for ((path, rel), released) in paths.iter().zip(&rels).zip(released) {
        let Some(head) = git::read_optional(path)? else {
            continue;
        };
        let at_base = base.base_bytes(&content::relative(path, base.workdir())?)?;
        let mut known = section_versions(&String::from_utf8_lossy(&at_base.unwrap_or_default()));
        known.extend(section_versions(released.as_deref().unwrap_or_default()));
        for version in section_versions(&String::from_utf8_lossy(&head)).difference(&known) {
            problems.push(format!(
                "{}: adds the section for {version}, which only the Version PR writes",
                rel.display()
            ));
        }
    }
    Ok(problems)
}

/// The version each `## name version` or `## version` heading names, as
/// [`crate::release::changelog_section_for`] finds them: its last word, when
/// that is a version. A heading whose name changes still names the same
/// version, so fixing a typo there adds none.
fn section_versions(changelog: &str) -> BTreeSet<Version> {
    changelog
        .lines()
        .filter_map(|line| line.trim().strip_prefix("## "))
        .filter_map(|heading| heading.split_whitespace().last())
        .filter_map(|word| Version::parse(word).ok())
        .collect()
}

/// Each package, declared at the merge-base and in the working tree, whose
/// publish setting differs between them while no added fragment bumps it.
///
/// A setting that HEAD's merge-base with the release branch already holds does
/// not count.
fn publish_changes(
    root: &Path,
    config: &Config,
    inputs: &Inputs,
    base: &MergeBase,
    release_candidates: &[String],
) -> Result<Vec<String>> {
    let config_rel = content::inside(base.workdir(), &inputs.config)?;
    let settings_changed = base.changed().iter().any(|path| {
        Path::new(path) == config_rel
            || Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    matches!(name, "Cargo.toml" | "package.json" | "pyproject.toml")
                })
    });
    if !settings_changed {
        return Ok(Vec::new());
    }
    let Some(before) = Snapshot::of(base, &config_rel)? else {
        return Ok(Vec::new());
    };
    let root = root.canonicalize().context("the config root")?;
    let bumped = bumped_by_added_fragments(base, inputs)?;
    let mut changed = Vec::new();
    for spec in &config.packages {
        let (Some(was), Some(now)) = (
            before.publishes(&spec.name)?,
            publishes(config, spec, &root, base.workdir())?,
        ) else {
            continue;
        };
        if was != now && !bumped.contains(&spec.name) {
            changed.push((spec.name.as_str(), was, now));
        }
    }
    if changed.is_empty() {
        return Ok(Vec::new());
    }
    let released = match MergeBase::read(&root, release_candidates)? {
        Some(released) => Snapshot::of(&released, &config_rel)?,
        None => None,
    };
    let mut problems = Vec::new();
    for (name, was, now) in changed {
        if let Some(released) = &released
            && released.publishes(name)?.as_ref() == Some(&now)
        {
            continue;
        }
        problems.push(format!(
            "{name}: {was} at the merge-base and {now} now, and no added fragment bumps it; add a fragment that bumps {name}, so the next Version PR releases it under the new setting",
        ));
    }
    Ok(problems)
}

/// The config at a merge-base, read from a scratch copy of its tree.
struct Snapshot {
    /// Holds the copy until the snapshot drops.
    scratch: Scratch,
    config: Config,
    root: PathBuf,
}

impl Snapshot {
    /// `None` when the merge-base has no file at `config_rel`.
    fn of(base: &MergeBase, config_rel: &Path) -> Result<Option<Self>> {
        let scratch = Scratch::new()?;
        base.write_to(scratch.path())?;
        let path = scratch.path().join(config_rel);
        if !path.is_file() {
            return Ok(None);
        }
        let config = Config::load(&path).context(content::BaseConfigFailed)?;
        let root = config::root_of(&path);
        Ok(Some(Self {
            scratch,
            config,
            root,
        }))
    }

    /// What the package `name` publishes here, or `None` when it is not
    /// declared or its manifest is missing.
    fn publishes(&self, name: &str) -> Result<Option<Publishes>> {
        let Ok(spec) = self.config.find(name) else {
            return Ok(None);
        };
        publishes(&self.config, spec, &self.root, self.scratch.path())
    }
}

/// The packages that a fragment added since the merge-base bumps above
/// `none`.
fn bumped_by_added_fragments(base: &MergeBase, inputs: &Inputs) -> Result<BTreeSet<String>> {
    let mut bumped = BTreeSet::new();
    for fragment in fragment::load_dir(&inputs.dir)? {
        let path = fragment
            .path
            .canonicalize()
            .with_context(|| fragment.path.display().to_string())?;
        let rel = content::relative(&path, base.workdir())?;
        if base.base_bytes(&rel)?.is_some() {
            continue;
        }
        bumped.extend(
            fragment
                .packages
                .iter()
                .filter(|package| package.bump != Bump::None)
                .map(|package| package.name.clone()),
        );
    }
    Ok(bumped)
}

/// Whether a package publishes: its `publisher`, the members a workspace
/// package uploads, and its manifest's own opt-out.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Publishes {
    /// It releases on GitHub only: `publisher: none`, or a manifest that opts
    /// out of its registry.
    No,
    /// It uploads to its registry.
    Yes,
    /// A cargo workspace package, which uploads these members.
    Members(BTreeSet<String>),
}

impl fmt::Display for Publishes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::No => formatter.write_str("publishes nothing"),
            Self::Yes => formatter.write_str("publishes"),
            Self::Members(members) if members.is_empty() => {
                formatter.write_str("publishes no member")
            }
            Self::Members(members) => {
                let names: Vec<&str> = members.iter().map(String::as_str).collect();
                write!(formatter, "publishes {}", names.join(", "))
            }
        }
    }
}

/// What a package publishes under `config`, rooted at `root`, or `None` when
/// its manifest is missing there. `tree` is the root of the tree it reads, and
/// no manifest lookup leaves it.
fn publishes(
    config: &Config,
    spec: &PackageSpec,
    root: &Path,
    tree: &Path,
) -> Result<Option<Publishes>> {
    if publisher::publisher_name(config, spec) == publisher::RELEASE_ONLY {
        return Ok(Some(Publishes::No));
    }
    if git::read_optional(&root.join(&spec.path))?.is_none() {
        return Ok(None);
    }
    let publisher = Publisher::of_in(config, spec, root, tree)?;
    if let Some(members) = publisher.members(config, spec, root)? {
        return Ok(Some(Publishes::Members(
            members.into_iter().map(|member| member.name).collect(),
        )));
    }
    Ok(Some(match publisher {
        Publisher::ReleaseOnly => Publishes::No,
        Publisher::Cargo { .. } | Publisher::Bun { .. } | Publisher::Command { .. } => {
            Publishes::Yes
        }
    }))
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::section_versions;

    #[test]
    fn a_section_is_read_by_the_version_its_heading_ends_with() {
        let versions = section_versions(indoc! {"
            # Changelog

            ## demo 1.2.0

            ## 1.1.0

            ## Unreleased
        "});
        let versions: Vec<String> = versions.iter().map(ToString::to_string).collect();
        assert_eq!(versions, ["1.1.0", "1.2.0"]);
    }
}
