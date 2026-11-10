//! What `check --versions` requires of a pull request that is not the Version
//! PR, beyond its versions.
//!
//! Such a pull request may not add a CHANGELOG section for a version: only the
//! Version PR writes one, for the version it moves to. And a pull request that
//! changes whether a package publishes must add a fragment that bumps it, so
//! the next Version PR releases that package under the new setting.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use semver::Version;
use toml_edit::{DocumentMut, Item};

use crate::config::{self, Config, PackageSpec};
use crate::content::{self, Inputs, Scratch};
use crate::fragment::{self, Bump};
use crate::git::{self, MergeBase};
use crate::publisher;

/// Each change against `base` that only the Version PR may make. One is a
/// CHANGELOG section for a version. Another is a package's publish setting
/// changed without an added fragment that bumps it. [`crate::versions::report`]
/// fails with them unless the pull request is the Version PR.
///
/// `base` is the merge-base with the pull request's base, and
/// `release_candidates` name the release branch. A section or a publish
/// setting that HEAD's merge-base with the release branch already holds came
/// with a merge of that branch, so it does not count.
///
/// # Errors
///
/// Returns an error when any of these cannot be read:
///
/// - the release branch's merge-base, or a CHANGELOG;
/// - the config at a merge-base;
/// - a manifest there or in the working tree, which must also parse.
pub fn problems(
    root: &Path,
    config: &Config,
    inputs: &Inputs,
    base: &MergeBase,
    release_candidates: &[String],
) -> Result<Vec<String>> {
    let mut problems = added_sections(root, config, base, release_candidates)?;
    problems.extend(publish_changes(
        root,
        config,
        inputs,
        base,
        release_candidates,
    )?);
    Ok(problems)
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
/// publish setting differs between them while no added fragment bumps it,
/// unless HEAD's merge-base with the release branch already holds the setting.
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
            publishes(config, spec, &root)?,
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
    _scratch: Scratch,
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
        let config = Config::load(&path).context("the config at a merge-base")?;
        let root = config::root_of(&path);
        Ok(Some(Self {
            _scratch: scratch,
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
        publishes(&self.config, spec, &self.root)
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
            cargo_opts_out(&manifest, &text).with_context(|| manifest.display().to_string())?
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

/// Whether the `Cargo.toml` at `manifest`, holding `text`, sets
/// `package.publish` to `false` or `[]`. With `publish.workspace = true` the
/// value is `workspace.package.publish` of its workspace root: the file
/// `package.workspace` names, else the same file when it holds a
/// `[workspace]`, else the nearest ancestor manifest that does, as Cargo
/// finds it.
fn cargo_opts_out(manifest: &Path, text: &str) -> Result<bool> {
    let document = text.parse::<DocumentMut>()?;
    let Some(package) = document.get("package") else {
        return Ok(false);
    };
    let Some(publish) = package.get("publish") else {
        return Ok(false);
    };
    if publish.get("workspace").and_then(Item::as_bool) != Some(true) {
        return Ok(denies(publish));
    }
    let Some(workspace) = workspace_root(manifest, &document)? else {
        return Ok(false);
    };
    Ok(workspace
        .get("workspace")
        .and_then(|table| table.get("package"))
        .and_then(|package| package.get("publish"))
        .is_some_and(denies))
}

/// `false`, or an empty list of registries.
fn denies(publish: &Item) -> bool {
    publish.as_bool() == Some(false) || publish.as_array().is_some_and(toml_edit::Array::is_empty)
}

/// The workspace root manifest of the package at `manifest`, parsed.
fn workspace_root(manifest: &Path, document: &DocumentMut) -> Result<Option<DocumentMut>> {
    if document.contains_key("workspace") {
        return Ok(Some(document.clone()));
    }
    let dir = manifest.parent().unwrap_or_else(|| Path::new("."));
    let named = document
        .get("package")
        .and_then(|package| package.get("workspace"))
        .and_then(Item::as_str);
    let candidates: Vec<PathBuf> = match named {
        Some(path) => vec![dir.join(path).join("Cargo.toml")],
        None => dir
            .ancestors()
            .skip(1)
            .map(|ancestor| ancestor.join("Cargo.toml"))
            .collect(),
    };
    for candidate in candidates {
        let Some(bytes) = git::read_optional(&candidate)? else {
            continue;
        };
        let text = String::from_utf8(bytes).with_context(|| candidate.display().to_string())?;
        let parsed = text
            .parse::<DocumentMut>()
            .with_context(|| candidate.display().to_string())?;
        if parsed.contains_key("workspace") {
            return Ok(Some(parsed));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use indoc::{formatdoc, indoc};
    use tempfile::TempDir;

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
        let opts_out = |text: &str| cargo_opts_out(Path::new("Cargo.toml"), text).expect("toml");
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

    /// A member that inherits `publish` reads it from the workspace root,
    /// as Cargo does.
    #[test]
    fn an_inherited_publish_reads_the_workspace_root() {
        let dir = TempDir::new().expect("tempdir");
        let member = dir.path().join("crates/a/Cargo.toml");
        fs::create_dir_all(member.parent().expect("parent")).expect("dir");
        let root = |line: &str| {
            fs::write(
                dir.path().join("Cargo.toml"),
                formatdoc! {r#"
                    [workspace]
                    members = ["crates/a"]

                    [workspace.package]
                    {line}
                "#},
            )
            .expect("write");
        };
        let text = indoc! {r#"
            [package]
            name = "a"
            publish.workspace = true
        "#};
        root("publish = false");
        assert!(cargo_opts_out(&member, text).expect("toml"));
        root(r#"edition = "2024""#);
        assert!(!cargo_opts_out(&member, text).expect("toml"));
    }
}
