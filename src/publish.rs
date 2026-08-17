//! Publish the versions already on HEAD.
//!
//! Stock commands: `cargo publish --locked` and
//! `bun publish --tolerate-republish`, then a GitHub Release.
//! JS packages are Bun, never `npm publish`. `registry = "github"` is
//! npm.pkg.github.com via `bunfig.toml` + `GITHUB_TOKEN`. A URL is
//! `--registry`. Cargo `registry` other than `crates-io` is
//! `cargo publish --registry`.

use crate::config::Config;
use crate::github;
use crate::process;
use anyhow::{Context, Result, bail};
use ctl_core::formatdoc;
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
    pub registry: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishKind {
    Cargo,
    Bun,
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
            registry: spec.registry.clone(),
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
                .map(|entry| {
                    let name = entry.name.as_str();
                    let version = entry.version.as_str();
                    let kind = entry.label();
                    formatdoc!("{name}@{version} ({kind})")
                })
                .collect(),
            release: Some(formatdoc!("would create {tag}", tag = planned.tag)),
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
    let argv = entry.command();
    match process::run_limited(&argv, &[], PUBLISH_TIMEOUT) {
        Ok(_) => Ok(coord),
        Err(error) if already_published(&format!("{error:#}")) => Ok(format!("{coord} (already)")),
        Err(error) => Err(error).context(coord),
    }
}

impl PublishEntry {
    /// Stock publish argv. Tests pin this instead of shelling out.
    #[must_use]
    pub fn command(&self) -> Vec<String> {
        match self.kind {
            PublishKind::Cargo => cargo_command(&self.path, self.registry.as_deref()),
            PublishKind::Bun => bun_command(&self.path, self.registry.as_deref()),
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        match (self.kind, self.registry.as_deref()) {
            (PublishKind::Cargo, None | Some("crates-io")) => "cargo".into(),
            (PublishKind::Cargo, Some(registry)) => format!("cargo {registry}"),
            (PublishKind::Bun, None | Some("npm")) => "bun".into(),
            (PublishKind::Bun, Some("github")) => "bun github".into(),
            (PublishKind::Bun, Some(registry)) => format!("bun {registry}"),
        }
    }
}

fn cargo_command(manifest: &Path, registry: Option<&str>) -> Vec<String> {
    let path = manifest.display().to_string();
    let mut cmd = vec![
        "cargo".into(),
        "publish".into(),
        "--locked".into(),
        "--manifest-path".into(),
        path,
    ];
    match registry {
        None | Some("crates-io") => {}
        Some(name) => {
            cmd.push("--registry".into());
            cmd.push(name.to_owned());
        }
    }
    cmd
}

fn bun_command(manifest: &Path, registry: Option<&str>) -> Vec<String> {
    let dir = manifest
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let cwd = dir.display().to_string();
    let mut cmd = vec![
        "bun".into(),
        "publish".into(),
        "--tolerate-republish".into(),
        "--cwd".into(),
        cwd,
    ];
    match registry {
        None | Some("npm") => {
            cmd.push("--access".into());
            cmd.push("public".into());
        }
        Some("github") => {
            cmd.push("--registry".into());
            cmd.push("https://npm.pkg.github.com".into());
        }
        Some(url) => {
            cmd.push("--registry".into());
            cmd.push(url.to_owned());
        }
    }
    cmd
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

fn kind_for(path: &Path) -> Result<PublishKind> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => Ok(PublishKind::Cargo),
        Some("package.json") => Ok(PublishKind::Bun),
        _ => bail!(
            "no stock publisher for {} (Cargo.toml or package.json)",
            path.display()
        ),
    }
}

impl PublishKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Bun => "bun",
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

    fn cargo() -> PublishEntry {
        PublishEntry {
            name: "ctl-core".into(),
            version: "0.0.1".into(),
            kind: PublishKind::Cargo,
            path: PathBuf::from("Cargo.toml"),
            registry: None,
        }
    }

    fn bun(registry: Option<&str>) -> PublishEntry {
        PublishEntry {
            name: "@org/pkg".into(),
            version: "0.0.1".into(),
            kind: PublishKind::Bun,
            path: PathBuf::from("packages/pkg/package.json"),
            registry: registry.map(str::to_owned),
        }
    }

    #[test]
    fn single_version_tag() {
        assert_eq!(tag_for(&[cargo()]), "v0.0.1");
    }

    #[test]
    fn cargo_argv() {
        assert_eq!(
            cargo().command(),
            [
                "cargo",
                "publish",
                "--locked",
                "--manifest-path",
                "Cargo.toml"
            ]
        );
        let mut alt = cargo();
        alt.registry = Some("my-index".into());
        assert_eq!(
            alt.command(),
            [
                "cargo",
                "publish",
                "--locked",
                "--manifest-path",
                "Cargo.toml",
                "--registry",
                "my-index",
            ]
        );
    }

    #[test]
    fn bun_argv_is_tolerate_republish() {
        assert_eq!(
            bun(None).command(),
            [
                "bun",
                "publish",
                "--tolerate-republish",
                "--cwd",
                "packages/pkg",
                "--access",
                "public",
            ]
        );
        assert_eq!(
            bun(Some("github")).command(),
            [
                "bun",
                "publish",
                "--tolerate-republish",
                "--cwd",
                "packages/pkg",
                "--registry",
                "https://npm.pkg.github.com",
            ]
        );
        assert_eq!(
            bun(Some("https://registry.example/npm/")).command()[6],
            "https://registry.example/npm/"
        );
    }

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
