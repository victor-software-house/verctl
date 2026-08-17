//! Compare declared manifest versions to the default branch.

use crate::config::Config;
use crate::git;
use crate::release::VERSION_BRANCH;
use anyhow::{Result, ensure};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    None,
    Ci,
    VersionBranch,
}

impl Skip {
    #[must_use]
    pub fn from_env(root: &Path) -> Self {
        if git::current_branch(root).as_deref() == Some(VERSION_BRANCH) {
            return Self::VersionBranch;
        }
        Self::None
    }

    #[must_use]
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Ci => Some("ci"),
            Self::VersionBranch => Some("version-packages"),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionRow {
    pub name: String,
    pub path: String,
    pub local: String,
    pub remote: Option<String>,
}

impl VersionRow {
    #[must_use]
    pub fn drifted(&self) -> bool {
        self.remote
            .as_deref()
            .is_some_and(|remote| remote != self.local)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionReport {
    pub skip: Option<String>,
    pub rows: Vec<VersionRow>,
}

impl VersionReport {
    #[must_use]
    pub fn drifted(&self) -> Vec<&VersionRow> {
        self.rows.iter().filter(|row| row.drifted()).collect()
    }

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

pub fn report(root: &Path, config: &Config) -> Result<VersionReport> {
    report_with(
        root,
        config,
        Skip::from_env(root),
        &git::default_branch_candidates(),
    )
}

pub fn report_with(
    root: &Path,
    config: &Config,
    skip: Skip,
    candidates: &[String],
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
    let mut rows = Vec::new();
    for (spec, remote_raw) in config.packages.iter().zip(remotes) {
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
        rows.push(VersionRow {
            name: spec.name.clone(),
            path: spec.path.display().to_string(),
            local,
            remote,
        });
    }
    Ok(VersionReport { skip: None, rows })
}

pub fn require(root: &Path, config: &Config) -> Result<VersionReport> {
    let report = report(root, config)?;
    report.require_clean()?;
    Ok(report)
}

pub fn require_with(
    root: &Path,
    config: &Config,
    skip: Skip,
    candidates: &[String],
) -> Result<VersionReport> {
    let report = report_with(root, config, skip, candidates)?;
    report.require_clean()?;
    Ok(report)
}

#[must_use]
pub fn stock_candidates() -> Vec<String> {
    git::default_branch_candidates_from(None)
}
