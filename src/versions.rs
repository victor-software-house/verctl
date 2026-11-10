//! Compare declared manifest versions to the default branch.

use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use crate::config::Config;
use crate::{content, git, pull_request, release};

/// Whether, and why, the version comparison is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Skip {
    /// Compare versions.
    None,
    /// Skip the comparison, reported as `ci`.
    Ci,
    /// Skip the comparison on the Version PR, where versions are meant to
    /// differ. [`report`] checks the Version PR's content instead.
    VersionBranch,
    /// Skip in a merge queue run. Each entry's pull request already compared
    /// its own versions, and a queue run combines entries, so a Version PR
    /// ahead in the queue changes the versions every later entry sees.
    MergeQueue,
}

impl Skip {
    /// Detect a Version PR from the current branch, when it is a
    /// `verctl/release/<base>` branch, or from the identity of the event's
    /// pull request, and a merge queue run from the event.
    ///
    /// # Errors
    ///
    /// Returns an error when the event names a pull request that cannot be
    /// read, as described by [`github::pull_request_is_version_pr`].
    ///
    /// [`github::pull_request_is_version_pr`]: crate::github::pull_request_is_version_pr
    pub fn from_env(root: &Path) -> Result<Self> {
        if on_version_branch(root) {
            return Ok(Self::VersionBranch);
        }
        if crate::github::event_is_merge_group() {
            return Ok(Self::MergeQueue);
        }
        // Detached Actions checkout: the pull request's head repository, head
        // branch, and live base, read from the API. Not GITHUB_HEAD_REF /
        // GITHUB_REF_NAME.
        if crate::github::pull_request_is_version_pr()? {
            return Ok(Self::VersionBranch);
        }
        Ok(Self::None)
    }

    /// The name reported for a skipped run, or `None` when nothing is skipped.
    #[must_use]
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Ci => Some("ci"),
            Self::VersionBranch => Some("version-pr"),
            Self::MergeQueue => Some("merge-queue"),
        }
    }
}

/// A package's version in the working tree beside its version at the
/// merge-base.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionRow {
    /// Package name from the config.
    pub name: String,
    /// Manifest path, relative to the directory holding `.ctl/`.
    pub path: String,
    /// Version in the working tree.
    pub local: String,
    /// Version at the merge-base, or `None` when the manifest is new.
    pub remote: Option<String>,
    /// Version at the merge-base of HEAD and the default branch, or `None`
    /// when the manifest is new there. It equals `remote` unless the pull
    /// request targets another branch.
    pub released: Option<String>,
}

impl VersionRow {
    /// Whether the working tree version differs from a merge-base that has
    /// one, and is not the version the default branch carries. Merging the
    /// default branch into another branch brings its released versions along,
    /// which is not a hand edit.
    #[must_use]
    pub fn drifted(&self) -> bool {
        self.remote
            .as_deref()
            .is_some_and(|remote| remote != self.local)
            && self.released.as_deref() != Some(self.local.as_str())
    }
}

/// The version rows of a run, or the reason the run was skipped.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionReport {
    /// The skip label, or `None` when versions were compared.
    pub skip: Option<String>,
    /// One row per package. A missing `VERSION` file in an existing directory
    /// reads as `0.0.0`; any other file a package cannot read fails the
    /// report, as it fails `version`.
    pub rows: Vec<VersionRow>,
}

impl VersionReport {
    /// Rows whose local version differs from the merge-base.
    #[must_use]
    pub fn drifted(&self) -> Vec<&VersionRow> {
        self.rows.iter().filter(|row| row.drifted()).collect()
    }

    /// Fail naming each drifted package, since versions change only through a
    /// fragment.
    ///
    /// # Errors
    ///
    /// Returns an error when any row has drifted from the merge-base. The
    /// message lists each drifted package with its remote and local version.
    pub fn require_clean(&self) -> Result<()> {
        let drifted = self.drifted();
        ensure!(
            drifted.is_empty(),
            "manifest versions differ from the merge-base of the default branch ({}); write a .changeset fragment instead of editing versions",
            drifted
                .iter()
                .map(|row| format!(
                    "{name} {remote} -> {local}",
                    name = row.name,
                    remote = row.remote.as_deref().unwrap_or("?"),
                    local = row.local
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(())
    }
}

/// Whether HEAD is on a `verctl/release/<base>` branch.
fn on_version_branch(root: &Path) -> bool {
    git::current_branch(root).is_some_and(|branch| release::is_version_branch(&branch))
}

/// Compare versions on HEAD to the merge-base. On the Version PR, check its
/// content instead with [`content::check`]: the versions are meant to differ
/// there, and every change must be one the version step makes. Any other pull
/// request also fails on each change [`pull_request::problems`] lists.
///
/// A pull request that changes no version, consumes no fragment, and has no
/// such problem never asks whether it is the Version PR, so it needs no token
/// and no API. One that does asks through [`Skip::from_env`]; a merge queue
/// run skips. On Actions, the base and default branches are fetched first when
/// the checkout lacks them or is shallow, as the depth-1 pull request checkout
/// is.
///
/// # Errors
///
/// Returns an error when that fetch fails, under the conditions of
/// [`report_with`], [`git::MergeBase::read`], [`pull_request::problems`],
/// and [`content::removes_fragment`], when [`Skip::from_env`] cannot read the
/// pull request, when the Version PR fails [`content::check`], or when another
/// pull request has a problem.
pub fn report(root: &Path, config: &Config, inputs: &content::Inputs) -> Result<VersionReport> {
    let fetch = git::FetchDefault::from_env();
    for branch in git::comparison_branches() {
        git::ensure_comparable(root, &branch, &fetch)?;
    }
    let candidates = git::default_branch_candidates();
    if on_version_branch(root) {
        content::check(root, config, inputs, &candidates)?;
        return report_with(root, config, Skip::VersionBranch, &[], &[]);
    }
    let release_candidates = git::release_branch_candidates();
    let report = report_with(root, config, Skip::None, &candidates, &release_candidates)?;
    let Some(base) = git::MergeBase::read(root, &candidates)? else {
        return Ok(report);
    };
    let problems = pull_request::problems(root, config, inputs, &base, &release_candidates)?;
    if problems.is_empty()
        && report.drifted().is_empty()
        && !content::removes_fragment(&base, inputs)?
    {
        return Ok(report);
    }
    let skip = Skip::from_env(root)?;
    match skip {
        Skip::VersionBranch => content::check(root, config, inputs, &candidates)?,
        Skip::None if !problems.is_empty() => bail!(
            "this pull request makes {count} change(s) only the Version PR may make:\n{list}",
            count = problems.len(),
            list = problems.join("\n"),
        ),
        Skip::None | Skip::Ci | Skip::MergeQueue => {}
    }
    Ok(match skip.label() {
        Some(label) => VersionReport {
            skip: Some(label.into()),
            rows: Vec::new(),
        },
        None => report,
    })
}

/// [`report`] with the skip given, and the candidates for the pull request's
/// base and for the default branch.
///
/// # Errors
///
/// Returns an error when either merge-base cannot be found or read, as
/// described for [`crate::git::files_on_merge_base`]. It also returns one,
/// naming the package, when its current version cannot be read (see
/// [`crate::config::PackageSpec::current`]) or its driver cannot read a
/// version from the merge-base or release copy.
pub fn report_with(
    root: &Path,
    config: &Config,
    skip: Skip,
    candidates: &[String],
    release_candidates: &[String],
) -> Result<VersionReport> {
    if let Some(label) = skip.label() {
        return Ok(VersionReport {
            skip: Some(label.into()),
            rows: Vec::new(),
        });
    }
    let rels: Vec<&Path> = config
        .packages
        .iter()
        .map(|spec| spec.path.as_path())
        .collect();
    let remotes = git::files_on_merge_base(root, &rels, candidates)?;
    let released = git::files_on_merge_base(root, &rels, release_candidates)?;
    let mut rows = Vec::new();
    for ((spec, remote_raw), released_raw) in config.packages.iter().zip(remotes).zip(released) {
        let current = spec.current(config, root)?;
        let context = || format!("package {}: {}", spec.name, spec.path.display());
        let read = |raw: &str| {
            current
                .driver
                .read(raw)
                .map(|version| version.trim().to_owned())
        };
        let remote = remote_raw
            .as_deref()
            .map(read)
            .transpose()
            .with_context(|| format!("{} on the merge-base", context()))?;
        let released = released_raw
            .as_deref()
            .map(read)
            .transpose()
            .with_context(|| format!("{} at the last release", context()))?;
        rows.push(VersionRow {
            name: spec.name.clone(),
            path: spec.path.display().to_string(),
            local: current.version,
            remote,
            released,
        });
    }
    Ok(VersionReport { skip: None, rows })
}

/// [`report`], failing when any version drifted.
///
/// # Errors
///
/// Returns an error under the conditions of [`report`], or when any version
/// drifted, as described for [`VersionReport::require_clean`].
pub fn require(root: &Path, config: &Config, inputs: &content::Inputs) -> Result<VersionReport> {
    let report = report(root, config, inputs)?;
    report.require_clean()?;
    Ok(report)
}

/// [`report_with`], failing when any version drifted.
///
/// # Errors
///
/// Returns an error under the conditions of [`report_with`], or when any
/// version drifted, as described for [`VersionReport::require_clean`].
pub fn require_with(
    root: &Path,
    config: &Config,
    skip: Skip,
    candidates: &[String],
    release_candidates: &[String],
) -> Result<VersionReport> {
    let report = report_with(root, config, skip, candidates, release_candidates)?;
    report.require_clean()?;
    Ok(report)
}

/// The default-branch candidates when no branch is configured.
#[must_use]
pub fn stock_candidates() -> Vec<String> {
    git::default_branch_candidates_from(None)
}
