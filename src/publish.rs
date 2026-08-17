//! Publish the versions already on HEAD. Cargo and npm stock drivers.
//!
//! OIDC trusted publishing is VER-007. This path uses the env token
//! `CARGO_REGISTRY_TOKEN` / `NPM_TOKEN` plus `GITHUB_TOKEN` for the
//! GitHub Release.

use crate::config::Config;
use crate::github;
use crate::process;
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::time::Duration;

pub const PUBLISH_TIMEOUT: Duration = Duration::from_mins(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPlan {
    pub packages: Vec<PublishEntry>,
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishEntry {
    pub name: String,
    pub version: String,
    pub kind: PublishKind,
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishKind {
    Cargo,
    Npm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOutcome {
    pub crates: Vec<String>,
    pub release: Option<String>,
}

pub fn plan(config: &Config, root: &Path) -> Result<PublishPlan> {
    let mut packages = Vec::new();
    for spec in &config.packages {
        let path = root.join(&spec.path);
        let kind = kind_for(&path)?;
        let driver = spec.resolve(config, root)?;
        let raw = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let version = driver.read(&raw)?.trim().to_owned();
        packages.push(PublishEntry {
            name: spec.name.clone(),
            version,
            kind,
            path,
        });
    }
    let tag = tag_for(&packages);
    Ok(PublishPlan { packages, tag })
}

pub fn run(config: &Config, root: &Path, dry_run: bool) -> Result<PublishOutcome> {
    let planned = plan(config, root)?;
    if dry_run {
        return Ok(PublishOutcome {
            crates: planned
                .packages
                .iter()
                .map(|entry| format!("{}@{} ({})", entry.name, entry.version, entry.kind.as_str()))
                .collect(),
            release: Some(format!("would create {}", planned.tag)),
        });
    }
    let token = crate::release::resolve_token()?;
    let mut crates = Vec::new();
    for entry in &planned.packages {
        crates.push(publish_package(entry)?);
    }
    let repo = github::repo(root)?;
    let notes = release_notes(root);
    let name = planned.packages.first().map_or_else(
        || planned.tag.clone(),
        |entry| format!("{} {}", entry.name, entry.version),
    );
    let release = github::ensure_release(&token, &repo, &planned.tag, &name, &notes)?;
    Ok(PublishOutcome {
        crates,
        release: Some(release),
    })
}

fn publish_package(entry: &PublishEntry) -> Result<String> {
    let coord = format!("{}@{}", entry.name, entry.version);
    match entry.kind {
        PublishKind::Cargo => {
            let manifest = entry.path.display().to_string();
            let argv =
                process::argv(&["cargo", "publish", "--locked", "--manifest-path", &manifest]);
            match process::run_limited(&argv, &[], PUBLISH_TIMEOUT) {
                Ok(_) => Ok(coord),
                Err(error) if already_published(&format!("{error:#}")) => {
                    Ok(format!("{coord} (already)"))
                }
                Err(error) => Err(error).context(coord),
            }
        }
        PublishKind::Npm => {
            let dir = entry
                .path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let prefix = dir.display().to_string();
            let argv =
                process::argv(&["npm", "publish", "--prefix", &prefix, "--access", "public"]);
            match process::run_limited(&argv, &[], PUBLISH_TIMEOUT) {
                Ok(_) => Ok(coord),
                Err(error) if already_published(&format!("{error:#}")) => {
                    Ok(format!("{coord} (already)"))
                }
                Err(error) => Err(error).context(coord),
            }
        }
    }
}

fn already_published(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("already uploaded")
        || lower.contains("already exists")
        || lower.contains("cannot be republished")
        || lower.contains("previously published")
}

fn kind_for(path: &Path) -> Result<PublishKind> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => Ok(PublishKind::Cargo),
        Some("package.json") => Ok(PublishKind::Npm),
        _ => bail!(
            "no stock publisher for {} (cargo or npm manifests only)",
            path.display()
        ),
    }
}

impl PublishKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
        }
    }
}

fn tag_for(packages: &[PublishEntry]) -> String {
    let mut versions: Vec<&str> = packages
        .iter()
        .map(|entry| entry.version.as_str())
        .collect();
    versions.sort_unstable();
    versions.dedup();
    match versions.as_slice() {
        [version] => format!("v{version}"),
        _ => packages
            .first()
            .map_or_else(|| "v0.0.0".into(), |entry| format!("v{}", entry.version)),
    }
}

fn release_notes(root: &Path) -> String {
    let Ok(body) = std::fs::read_to_string(root.join("CHANGELOG.md")) else {
        return String::new();
    };
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("## ") {
            let mut section = String::from(line);
            section.push('\n');
            for next in lines {
                if next.starts_with("## ") {
                    break;
                }
                section.push_str(next);
                section.push('\n');
            }
            return section;
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::{PublishEntry, PublishKind, already_published, tag_for};
    use std::path::PathBuf;

    #[test]
    fn single_version_tag() {
        let packages = [PublishEntry {
            name: "ctl-core".into(),
            version: "0.0.1".into(),
            kind: PublishKind::Cargo,
            path: PathBuf::from("Cargo.toml"),
        }];
        assert_eq!(tag_for(&packages), "v0.0.1");
    }

    #[test]
    fn already_uploaded_is_ok() {
        assert!(already_published(
            "crate ctl-core@0.0.1 already exists on crates.io"
        ));
        assert!(already_published("error: crate already uploaded"));
        assert!(!already_published("token is required"));
    }
}
