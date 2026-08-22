//! Publish the versions already on HEAD, then open a GitHub Release.
//!
//! How each package is published comes from `[publishers.*]`. Cargo and
//! bun are stock recipes, not the only stacks. How tags are named comes
//! from `[tags].template` — see [`tags_for`].

use crate::config::{Config, NAME_PLACEHOLDER};
use crate::git;
use crate::github;
use crate::process;
use crate::publisher;
use crate::release;
use anyhow::{Context, Result, bail, ensure};
use ctl_core::formatdoc;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

pub const PUBLISH_TIMEOUT: Duration = Duration::from_mins(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPlan {
    pub packages: Vec<PublishEntry>,
    pub tags: Vec<TagPlan>,
}

/// One tag the release will create, and the packages whose notes fill it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPlan {
    pub tag: String,
    pub title: String,
    pub packages: Vec<PublishEntry>,
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
    pub releases: Vec<String>,
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
    let tags = tags_for(config, &packages)?;
    Ok(PublishPlan { packages, tags })
}

pub fn run(config: &Config, root: &Path, dry_run: bool) -> Result<PublishOutcome> {
    let planned = plan(config, root)?;
    if dry_run {
        let releases = planned
            .tags
            .iter()
            .map(|tag| formatdoc!("would create {tag}", tag = tag.tag))
            .collect();
        return Ok(PublishOutcome {
            packages: planned.packages.iter().map(PublishLine::from).collect(),
            releases,
        });
    }
    prove_version_commit(config, root, &planned)?;
    let token = release::resolve_token()?;
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
    let sha = git::head_sha(root)?;
    let mut releases = Vec::new();
    for tag in &planned.tags {
        let notes = release::notes_for(
            config,
            root,
            tag.packages
                .iter()
                .map(|entry| (entry.name.as_str(), entry.version.as_str())),
        );
        let url = github::ensure_release(&token, &repo, &tag.tag, &tag.title, &notes, &sha)?;
        releases.push(url);
    }
    Ok(PublishOutcome { packages, releases })
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

fn prove_version_commit(config: &Config, root: &Path, planned: &PublishPlan) -> Result<()> {
    release::require_changelog_versions(
        config,
        root,
        planned
            .packages
            .iter()
            .map(|entry| (entry.name.as_str(), entry.version.as_str())),
    )?;
    git::require_on_default_history(root)
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

/// Render `[tags].template` over the packages that will ship.
///
/// A template without `{name}` names one tag for the whole release and refuses
/// when the packages do not share one version. A template with `{name}` names
/// one tag per package and refuses when two packages would render the same tag.
pub fn tags_for(config: &Config, packages: &[PublishEntry]) -> Result<Vec<TagPlan>> {
    ensure!(
        !packages.is_empty(),
        "the config declares no packages to publish"
    );
    if config.tags.uses_name() {
        return per_package_tags(config, packages);
    }
    one_shared_tag(config, packages)
}

fn one_shared_tag(config: &Config, packages: &[PublishEntry]) -> Result<Vec<TagPlan>> {
    let template = &config.tags.template;
    let mut by_version: BTreeMap<&str, Vec<&PublishEntry>> = BTreeMap::new();
    for entry in packages {
        by_version
            .entry(entry.version.as_str())
            .or_default()
            .push(entry);
    }
    if by_version.len() > 1 {
        let detail = packages
            .iter()
            .map(|entry| format!("{}@{}", entry.name, entry.version))
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "tags.template {template:?} has no {NAME_PLACEHOLDER}, so it can only name one tag, but this release has more than one version: {detail}"
        );
    }
    let version = packages[0].version.as_str();
    let tag = config.tags.render(version, None)?;
    let title = packages.first().map_or_else(
        || tag.clone(),
        |entry| format!("{} {}", entry.name, entry.version),
    );
    Ok(vec![TagPlan {
        tag,
        title,
        packages: packages.to_vec(),
    }])
}

fn per_package_tags(config: &Config, packages: &[PublishEntry]) -> Result<Vec<TagPlan>> {
    let template = &config.tags.template;
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut tags = Vec::with_capacity(packages.len());
    for entry in packages {
        let tag = config.tags.render(&entry.version, Some(&entry.name))?;
        if let Some(earlier) = seen.insert(tag.clone(), entry.name.clone()) {
            bail!(
                "tags.template {template:?} renders the same tag {tag:?} for {earlier} and {}",
                entry.name
            );
        }
        tags.push(TagPlan {
            tag,
            title: format!("{} {}", entry.name, entry.version),
            packages: vec![entry.clone()],
        });
    }
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::{PublishEntry, already_published, tags_for};
    use crate::config::Config;
    use indoc::indoc;

    fn entry(name: &str, version: &str) -> PublishEntry {
        PublishEntry {
            name: name.into(),
            version: version.into(),
            noun: "crate".into(),
            label: "cargo".into(),
            argv: vec!["true".into()],
        }
    }

    fn config(tags: &str) -> Config {
        let body = if tags.is_empty() {
            indoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
            "}
            .to_owned()
        } else {
            formatdoc_config(tags)
        };
        Config::parse(&body).unwrap_or_else(|error| panic!("{error:#}"))
    }

    fn formatdoc_config(tags: &str) -> String {
        use ctl_core::formatdoc;
        formatdoc! {"
            packages:
              - name: demo
                path: Cargo.toml

            tags:
              template: {tags:?}
        "}
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

    #[test]
    fn default_template_is_one_v_version_tag() {
        let planned = tags_for(&config(""), &[entry("demo", "1.2.3")]).unwrap();
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].tag, "v1.2.3");
        assert_eq!(planned[0].packages.len(), 1);
    }

    #[test]
    fn shared_version_across_packages_is_one_tag() {
        let planned = tags_for(&config(""), &[entry("a", "1.0.0"), entry("b", "1.0.0")]).unwrap();
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].tag, "v1.0.0");
        assert_eq!(planned[0].packages.len(), 2);
    }

    #[test]
    fn differing_versions_without_name_refuse() {
        let error = tags_for(&config(""), &[entry("a", "1.0.0"), entry("b", "2.0.0")])
            .expect_err("must refuse");
        let message = format!("{error:#}");
        assert!(message.contains("{name}"), "{message}");
        assert!(message.contains("a@1.0.0"), "{message}");
        assert!(message.contains("b@2.0.0"), "{message}");
    }

    #[test]
    fn per_package_template_names_each_tag() {
        let planned = tags_for(
            &config("{name}@{version}"),
            &[entry("a", "1.0.0"), entry("b", "2.0.0")],
        )
        .unwrap();
        assert_eq!(
            planned
                .iter()
                .map(|tag| tag.tag.as_str())
                .collect::<Vec<_>>(),
            ["a@1.0.0", "b@2.0.0"]
        );
        assert!(planned.iter().all(|tag| tag.packages.len() == 1));
    }

    #[test]
    fn same_version_with_name_still_one_tag_each() {
        let planned = tags_for(
            &config("{name}@{version}"),
            &[entry("a", "1.0.0"), entry("b", "1.0.0")],
        )
        .unwrap();
        assert_eq!(
            planned
                .iter()
                .map(|tag| tag.tag.as_str())
                .collect::<Vec<_>>(),
            ["a@1.0.0", "b@1.0.0"]
        );
    }

    #[test]
    fn colliding_per_package_template_refuses() {
        let error = tags_for(
            &config("{name}@{version}"),
            &[entry("dup", "1.0.0"), entry("dup", "1.0.0")],
        )
        .expect_err("same rendered tag");
        assert!(format!("{error:#}").contains("same tag"), "{error:#}");
    }
}
