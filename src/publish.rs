//! Publish the versions already on HEAD, then open a GitHub Release.
//!
//! How each package is published comes from `[publishers.*]`. Cargo and
//! bun are stock recipes, not the only stacks.

use crate::config::Config;
use crate::github;
use crate::process;
use crate::publisher;
use anyhow::{Context, Result};
use ctl_core::formatdoc;
use serde::Serialize;
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
    pub noun: String,
    pub label: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOutcome {
    pub packages: Vec<PublishLine>,
    pub release: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublishLine {
    pub name: String,
    pub version: String,
    pub via: String,
    pub note: Option<String>,
}

pub fn plan(config: &Config, root: &Path) -> Result<PublishPlan> {
    let mut packages = Vec::new();
    for spec in &config.packages {
        let path = root.join(&spec.path);
        let driver = spec.resolve(config, root)?;
        let raw = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let version = driver.read(&raw)?.trim().to_owned();
        let (label, noun, argv) = publisher::resolve(config, spec, &path)?;
        let label = match spec.registry.as_deref() {
            Some(registry) if !registry.is_empty() => format!("{label} {registry}"),
            _ => label,
        };
        packages.push(PublishEntry {
            name: spec.name.clone(),
            version,
            noun,
            label,
            argv,
        });
    }
    let tag = tag_for(&packages);
    Ok(PublishPlan { packages, tag })
}

pub fn run(config: &Config, root: &Path, dry_run: bool) -> Result<PublishOutcome> {
    let planned = plan(config, root)?;
    if dry_run {
        return Ok(PublishOutcome {
            packages: planned.packages.iter().map(PublishLine::from).collect(),
            release: Some(formatdoc!("would create {tag}", tag = planned.tag)),
        });
    }
    let token = crate::release::resolve_token()?;
    let mut packages = Vec::new();
    for entry in &planned.packages {
        let note = match publish_package(entry)? {
            status if status.ends_with(" (already)") => Some("already".into()),
            _ => None,
        };
        packages.push(PublishLine {
            name: entry.name.clone(),
            version: entry.version.clone(),
            via: entry.label.clone(),
            note,
        });
    }
    let repo = github::repo(root)?;
    let notes = release_notes(root);
    let name = planned.packages.first().map_or_else(
        || planned.tag.clone(),
        |entry| format!("{} {}", entry.name, entry.version),
    );
    let release = github::ensure_release(&token, &repo, &planned.tag, &name, &notes)?;
    Ok(PublishOutcome {
        packages,
        release: Some(release),
    })
}

fn publish_package(entry: &PublishEntry) -> Result<String> {
    let coord = format!("{}@{}", entry.name, entry.version);
    match process::run_inherit(&entry.argv, PUBLISH_TIMEOUT) {
        Ok(()) => Ok(coord),
        Err(error) if already_published(&format!("{error:#}")) => Ok(format!("{coord} (already)")),
        Err(error) => Err(error).context(coord),
    }
}

impl From<&PublishEntry> for PublishLine {
    fn from(entry: &PublishEntry) -> Self {
        Self {
            name: entry.name.clone(),
            version: entry.version.clone(),
            via: entry.label.clone(),
            note: None,
        }
    }
}

fn already_published(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("already uploaded")
        || lower.contains("already exists")
        || lower.contains("cannot be republished")
        || lower.contains("previously published")
        || lower.contains("tolerate-republish")
        || lower.contains("version already exists")
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
    use super::already_published;

    #[test]
    fn already_uploaded_is_ok() {
        assert!(already_published(
            "crate ctl-core@0.0.1 already exists on crates.io"
        ));
        assert!(already_published("error: crate already uploaded"));
        assert!(already_published("version already exists"));
        assert!(!already_published("token is required"));
    }
}
