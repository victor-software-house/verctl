//! Compare declared manifest versions to the default branch.

use std::fs;
use std::path::Path;

use anyhow::{Result, ensure};
use serde::Serialize;

use crate::config::Config;
use crate::{git, release};

/// Whether, and why, the version comparison is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// Compare versions.
    None,
    /// Skip the comparison, reported as `ci`.
    Ci,
    /// Skip on the Version PR, where versions are meant to differ.
    VersionBranch,
}

impl Skip {
    /// Detect a Version PR from the current branch, when it is a
    /// `verctl/release/<base>` branch, or from the identity of the event's
    /// pull request.
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
    /// One row per package whose manifest exists in the working tree.
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

/// Compare versions on HEAD to the merge-base, skipping per [`Skip::from_env`]
/// when a version drifted. A local Version PR branch skips before reading
/// anything. A pull request that changes no version never asks whether it is
/// a Version PR, so it needs no token and no API. On Actions, the base and
/// default branches are fetched first when the checkout lacks them or is
/// shallow, as the depth-1 pull request checkout is.
///
/// # Errors
///
/// Returns an error when that fetch fails, under the conditions of
/// [`report_with`], or when a version drifted and [`Skip::from_env`] cannot
/// read the pull request.
pub fn report(root: &Path, config: &Config) -> Result<VersionReport> {
    if on_version_branch(root) {
        return report_with(root, config, Skip::VersionBranch, &[], &[]);
    }
    let fetch = git::FetchDefault::from_env();
    for branch in git::comparison_branches() {
        git::ensure_comparable(root, &branch, &fetch)?;
    }
    let report = report_with(
        root,
        config,
        Skip::None,
        &git::default_branch_candidates(),
        &git::release_branch_candidates(),
    )?;
    if report.drifted().is_empty() {
        return Ok(report);
    }
    Ok(match Skip::from_env(root)?.label() {
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
/// described for [`crate::git::files_on_merge_base`], when a package's driver
/// cannot be resolved, or when the driver cannot read a version from the local
/// manifest or from the merge-base copy.
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
        let path = root.join(&spec.path);
        let Ok(local_raw) = fs::read_to_string(&path) else {
            continue;
        };
        let driver = spec.resolve(config, root)?;
        let local = driver.read(&local_raw)?;
        let remote = remote_raw
            .as_deref()
            .map(|raw| driver.read(raw))
            .transpose()?;
        let released = released_raw
            .as_deref()
            .map(|raw| driver.read(raw))
            .transpose()?;
        rows.push(VersionRow {
            name: spec.name.clone(),
            path: spec.path.display().to_string(),
            local,
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
pub fn require(root: &Path, config: &Config) -> Result<VersionReport> {
    let report = report(root, config)?;
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
