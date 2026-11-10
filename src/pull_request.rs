//! What `check --versions` requires of a pull request that is not the Version
//! PR, beyond its versions.
//!
//! Such a pull request may not add a CHANGELOG section for a version: only the
//! Version PR writes one, for the version it moves to. And a pull request that
//! changes whether a package publishes must add a fragment that bumps it, so
//! the next Version PR releases that package under the new setting.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use semver::Version;
use toml_edit::{DocumentMut, Item};

use crate::config::{self, Config, PackageSpec};
use crate::content::{self, Inputs, Scratch};
use crate::fragment::{self, Bump};
use crate::git::{self, MergeBase};
use crate::publisher;

/// Each change the working tree makes that only the Version PR may make: a
/// CHANGELOG section for a version, and a package's publish setting changed
/// without an added fragment that bumps it. [`crate::versions::report`] fails
/// with them unless the pull request is the Version PR.
///
/// `candidates` name the pull request's base, and `release_candidates` the
/// release branch. A section the CHANGELOG already holds where HEAD branched
/// from the release branch came with a merge of that branch, so it does not
/// count.
///
/// # Errors
///
/// Returns an error when a merge-base or a CHANGELOG cannot be read, when the
/// config at the merge-base cannot be loaded, or when a manifest there or in
/// the working tree cannot be read or parsed.
pub fn problems(
    root: &Path,
    config: &Config,
    inputs: &Inputs,
    candidates: &[String],
    release_candidates: &[String],
) -> Result<Vec<String>> {
    let mut problems = added_sections(root, config, candidates, release_candidates)?;
    problems.extend(publish_changes(root, config, inputs, candidates)?);
    Ok(problems)
}

/// Each version section the working tree adds to a package CHANGELOG.
fn added_sections(
    root: &Path,
    config: &Config,
    candidates: &[String],
    release_candidates: &[String],
) -> Result<Vec<String>> {
    let mut rels = BTreeSet::new();
    for spec in &config.packages {
        let path = spec.changelog_path(root);
        let rel = path
            .strip_prefix(root)
            .with_context(|| format!("{} is outside {}", path.display(), root.display()))?;
        rels.insert(rel.to_path_buf());
    }
    let rels: Vec<&Path> = rels.iter().map(std::path::PathBuf::as_path).collect();
    let base = git::files_on_merge_base(root, &rels, candidates)?;
    let released = git::files_on_merge_base(root, &rels, release_candidates)?;
    let mut problems = Vec::new();
    for ((rel, base), released) in rels.iter().zip(base).zip(released) {
        let Some(head) = git::read_optional(&root.join(rel))? else {
            continue;
        };
        let head = String::from_utf8_lossy(&head);
        let mut known = section_versions(base.as_deref().unwrap_or_default());
        known.extend(section_versions(released.as_deref().unwrap_or_default()));
        for version in section_versions(&head).difference(&known) {
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
fn publish_changes(
    root: &Path,
    config: &Config,
    inputs: &Inputs,
    candidates: &[String],
) -> Result<Vec<String>> {
    let Some(base) = MergeBase::read(root, candidates)? else {
        return Ok(Vec::new());
    };
    let workdir = base.workdir();
    let config_rel = content::inside(workdir, &inputs.config)?;
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
    let scratch = Scratch::new()?;
    base.write_to(scratch.path())?;
    let base_config_path = scratch.path().join(&config_rel);
    if !base_config_path.is_file() {
        return Ok(Vec::new());
    }
    let base_config = Config::load(&base_config_path).context("the config at the merge-base")?;
    let base_root = config::root_of(&base_config_path);
    let root = root.canonicalize().context("the config root")?;
    let bumped = bumped_by_added_fragments(&base, inputs)?;
    let mut problems = Vec::new();
    for spec in &config.packages {
        let Ok(before_spec) = base_config.find(&spec.name) else {
            continue;
        };
        let (Some(before), Some(after)) = (
            publishes(&base_config, before_spec, &base_root)?,
            publishes(config, spec, &root)?,
        ) else {
            continue;
        };
        if before != after && !bumped.contains(&spec.name) {
            problems.push(format!(
                "{name}: {before} at the merge-base and {after} now, and no added fragment bumps it; add a fragment that bumps {name}, so the next Version PR releases it under the new setting",
                name = spec.name,
            ));
        }
    }
    Ok(problems)
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

/// What a package publishes under `config` in the tree at `root`, or `None`
/// when its manifest is missing there.
fn publishes(config: &Config, spec: &PackageSpec, root: &Path) -> Result<Option<Publishes>> {
    if publisher::publisher_name(config, spec) == publisher::RELEASE_ONLY {
        return Ok(Some(Publishes::No));
    }
    let manifest = root.join(&spec.path);
    let Some(bytes) = git::read_optional(&manifest)? else {
        return Ok(None);
    };
    if let Some(members) = publisher::workspace_members(config, spec, root)? {
        return Ok(Some(Publishes::Members(
            members.into_iter().map(|member| member.name).collect(),
        )));
    }
    let text = String::from_utf8(bytes).with_context(|| manifest.display().to_string())?;
    let opts_out = match manifest.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => {
            cargo_opts_out(&text).with_context(|| manifest.display().to_string())?
        }
        Some("package.json") => {
            let json: serde_json::Value =
                serde_json::from_str(&text).with_context(|| manifest.display().to_string())?;
            json.get("private") == Some(&serde_json::Value::Bool(true))
        }
        _ => false,
    };
    Ok(Some(if opts_out {
        Publishes::No
    } else {
        Publishes::Yes
    }))
}

/// Whether a `Cargo.toml` sets `package.publish` to `false` or `[]`, directly
/// or through `publish.workspace = true` and the same file's
/// `workspace.package.publish`.
fn cargo_opts_out(text: &str) -> Result<bool> {
    let document = text.parse::<DocumentMut>()?;
    let Some(publish) = document
        .get("package")
        .and_then(|package| package.get("publish"))
    else {
        return Ok(false);
    };
    let inherits = publish
        .get("workspace")
        .and_then(Item::as_bool)
        .unwrap_or(false);
    let publish = if inherits {
        match document
            .get("workspace")
            .and_then(|workspace| workspace.get("package"))
            .and_then(|package| package.get("publish"))
        {
            Some(publish) => publish,
            None => return Ok(false),
        }
    } else {
        publish
    };
    Ok(publish.as_bool() == Some(false)
        || publish.as_array().is_some_and(toml_edit::Array::is_empty))
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::{cargo_opts_out, section_versions};

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

    #[test]
    fn publish_false_and_an_empty_list_opt_out() {
        let opts_out = |text: &str| cargo_opts_out(text).expect("toml");
        assert!(opts_out(indoc! {r#"
            [package]
            name = "a"
            publish = false
        "#}));
        assert!(opts_out(indoc! {r#"
            [package]
            name = "a"
            publish = []
        "#}));
        assert!(!opts_out(indoc! {r#"
            [package]
            name = "a"
            publish = ["crates-io"]
        "#}));
        assert!(opts_out(indoc! {r#"
            [package]
            name = "a"
            publish.workspace = true

            [workspace.package]
            publish = false
        "#}));
    }
}
