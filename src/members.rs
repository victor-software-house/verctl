//! The members a virtual cargo workspace publishes, in publish order.
//!
//! A package whose manifest is a virtual workspace publishes one member at a
//! time, so a rerun after a partial release uploads only what is missing:
//! `cargo publish --workspace` refuses to run when any selected version
//! exists. Cargo resolves member globs and workspace inheritance, so verctl
//! reads only `cargo metadata`:
//! <https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use toml_edit::DocumentMut;

use crate::process;

/// How long `cargo metadata --no-deps` may run. It reads only manifests.
const METADATA_TIMEOUT: Duration = Duration::from_mins(2);

/// A workspace member that publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Member {
    /// The crate name.
    pub name: String,
    /// The version Cargo resolves for it.
    pub version: String,
    /// Its `Cargo.toml`.
    pub manifest: PathBuf,
    /// The registries its `publish` names. `None` when it names none, which
    /// lets it publish anywhere.
    pub publish: Option<Vec<String>>,
}

/// Whether `manifest` is a virtual workspace manifest: a `[workspace]` table
/// and no `[package]` table.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read or is not TOML.
pub fn is_virtual(manifest: &Path) -> Result<bool> {
    let raw = std::fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
    let document = raw
        .parse::<DocumentMut>()
        .with_context(|| manifest.display().to_string())?;
    Ok(document.contains_key("workspace") && !document.contains_key("package"))
}

/// The members of the workspace at `manifest` that publish, each after the
/// published members it depends on.
///
/// A member with `publish = false` is left out. A normal or build dependency
/// orders publishes, and so does a development dependency that declares a
/// version, which `cargo publish` keeps; a path-only development dependency
/// does not, since `cargo publish` strips it. Members with no such dependency
/// between them keep the order of `workspace_members`.
///
/// # Errors
///
/// Returns an error when `cargo metadata` fails or prints something other than
/// its JSON, or when the dependencies that order publishes form a cycle, which
/// cargo cannot publish (<https://github.com/rust-lang/cargo/issues/4242>).
pub fn members(manifest: &Path) -> Result<Vec<Member>> {
    let metadata: Metadata =
        serde_json::from_str(&metadata(manifest)?).context("cargo metadata output")?;
    in_publish_order(&metadata)
}

/// Whether the package `name`, in the workspace of `manifest`, has a target
/// cargo-semver-checks can check. A crate with only binaries has no API.
///
/// # Errors
///
/// Returns an error when `cargo metadata` fails or prints something other than
/// its JSON, or when it lists no package `name`.
pub fn has_library(manifest: &Path, name: &str) -> Result<bool> {
    let metadata: TargetMetadata =
        serde_json::from_str(&metadata(manifest)?).context("cargo metadata output")?;
    library_in(&metadata, name).with_context(|| manifest.display().to_string())
}

/// `cargo metadata --no-deps` for the workspace of `manifest`.
fn metadata(manifest: &Path) -> Result<String> {
    let path = manifest.display().to_string();
    let argv = process::argv(&[
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--manifest-path",
        &path,
    ]);
    process::run_limited(&argv, &[], METADATA_TIMEOUT)
}

/// The target kinds cargo-semver-checks 0.51.0 checks
/// (`is_lib_like_checkable_target`): `lib` and the explicit crate types
/// besides `proc-macro`.
const LIBRARY_KINDS: [&str; 5] = ["lib", "rlib", "dylib", "cdylib", "staticlib"];

/// The parts of `cargo metadata --format-version 1` [`has_library`] reads.
#[derive(Deserialize)]
struct TargetMetadata {
    packages: Vec<TargetPackage>,
}

#[derive(Deserialize)]
struct TargetPackage {
    name: String,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    kind: Vec<String>,
}

fn library_in(metadata: &TargetMetadata, name: &str) -> Result<bool> {
    let package = metadata
        .packages
        .iter()
        .find(|package| package.name == name)
        .ok_or_else(|| anyhow::anyhow!("cargo metadata lists no package {name}"))?;
    Ok(package
        .targets
        .iter()
        .flat_map(|target| &target.kind)
        .any(|kind| LIBRARY_KINDS.contains(&kind.as_str())))
}

/// The parts of `cargo metadata --format-version 1` this module reads.
#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    version: String,
    manifest_path: PathBuf,
    publish: Option<Vec<String>>,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    req: String,
    kind: Option<DependencyKind>,
}

/// Cargo's dependency kinds besides normal, which `cargo metadata` writes as
/// `null`.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum DependencyKind {
    Dev,
    Build,
}

impl Dependency {
    /// Whether `cargo publish` keeps this dependency, so its crate must be on
    /// the registry first. A path-only dependency has the requirement `*`.
    fn orders_publish(&self) -> bool {
        match self.kind {
            None | Some(DependencyKind::Build) => true,
            Some(DependencyKind::Dev) => self.req != "*",
        }
    }
}

/// Where a member stands in the depth-first walk that orders publishes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Visiting,
    Done,
}

fn in_publish_order(metadata: &Metadata) -> Result<Vec<Member>> {
    let by_id: BTreeMap<&str, &Package> = metadata
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package))
        .collect();
    let published: Vec<&Package> = metadata
        .workspace_members
        .iter()
        .filter_map(|id| by_id.get(id.as_str()).copied())
        .filter(|package| {
            package
                .publish
                .as_ref()
                .is_none_or(|names| !names.is_empty())
        })
        .collect();
    let index: BTreeMap<&str, usize> = published
        .iter()
        .enumerate()
        .map(|(at, package)| (package.name.as_str(), at))
        .collect();
    let edges: Vec<Vec<usize>> = published
        .iter()
        .map(|package| {
            package
                .dependencies
                .iter()
                .filter(|dependency| dependency.orders_publish())
                .filter_map(|dependency| index.get(dependency.name.as_str()).copied())
                .collect()
        })
        .collect();
    let mut marks = vec![None; published.len()];
    let mut path = Vec::new();
    let mut order = Vec::with_capacity(published.len());
    for start in 0..published.len() {
        visit(start, &edges, &mut marks, &mut path, &mut order).map_err(|cycle| {
            let names: Vec<&str> = cycle
                .iter()
                .map(|&at| published[at].name.as_str())
                .collect();
            anyhow::anyhow!(
                "workspace members depend on each other in a cycle, which cargo cannot publish: {}; a path-only development dependency breaks it",
                names.join(" -> ")
            )
        })?;
    }
    Ok(order
        .into_iter()
        .map(|at| {
            let package = published[at];
            Member {
                name: package.name.clone(),
                version: package.version.clone(),
                manifest: package.manifest_path.clone(),
                publish: package.publish.clone(),
            }
        })
        .collect())
}

/// Append `at` to `order` after everything it depends on. On a cycle, return
/// it as member indices, first and last the same.
fn visit(
    at: usize,
    edges: &[Vec<usize>],
    marks: &mut [Option<Mark>],
    path: &mut Vec<usize>,
    order: &mut Vec<usize>,
) -> std::result::Result<(), Vec<usize>> {
    match marks[at] {
        Some(Mark::Done) => return Ok(()),
        Some(Mark::Visiting) => {
            let from = path.iter().position(|&on| on == at).unwrap_or_default();
            let mut cycle = path[from..].to_vec();
            cycle.push(at);
            return Err(cycle);
        }
        None => {}
    }
    marks[at] = Some(Mark::Visiting);
    path.push(at);
    for &next in &edges[at] {
        visit(next, edges, marks, path, order)?;
    }
    path.pop();
    marks[at] = Some(Mark::Done);
    order.push(at);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use indoc::indoc;
    use tempfile::TempDir;

    use super::{
        Metadata, TargetMetadata, has_library, in_publish_order, is_virtual, library_in, members,
    };

    fn names(json: &str) -> Vec<String> {
        let metadata: Metadata = serde_json::from_str(json).unwrap();
        in_publish_order(&metadata)
            .unwrap()
            .into_iter()
            .map(|member| member.name)
            .collect()
    }

    #[test]
    fn a_dependent_listed_first_publishes_after_its_dependency() {
        let found = names(indoc! {r#"
            {
              "workspace_members": ["app", "core"],
              "packages": [
                {"id": "app", "name": "app", "version": "0.0.2", "manifest_path": "/w/app/Cargo.toml", "publish": null,
                 "dependencies": [{"name": "core", "req": "=0.0.2", "kind": null}]},
                {"id": "core", "name": "core", "version": "0.0.2", "manifest_path": "/w/core/Cargo.toml", "publish": null,
                 "dependencies": []}
              ]
            }
        "#});
        assert_eq!(found, ["core", "app"]);
    }

    #[test]
    fn a_path_only_dev_dependency_does_not_order_publishes() {
        let found = names(indoc! {r#"
            {
              "workspace_members": ["core", "app"],
              "packages": [
                {"id": "core", "name": "core", "version": "0.0.2", "manifest_path": "/w/core/Cargo.toml", "publish": null,
                 "dependencies": [{"name": "app", "req": "*", "kind": "dev"}]},
                {"id": "app", "name": "app", "version": "0.0.2", "manifest_path": "/w/app/Cargo.toml", "publish": null,
                 "dependencies": [{"name": "core", "req": "=0.0.2", "kind": null}]}
              ]
            }
        "#});
        assert_eq!(found, ["core", "app"]);
    }

    #[test]
    fn a_member_with_publish_false_is_left_out() {
        let found = names(indoc! {r#"
            {
              "workspace_members": ["core", "xtask"],
              "packages": [
                {"id": "core", "name": "core", "version": "0.0.2", "manifest_path": "/w/core/Cargo.toml", "publish": ["crates-io"],
                 "dependencies": []},
                {"id": "xtask", "name": "xtask", "version": "0.1.0", "manifest_path": "/w/xtask/Cargo.toml", "publish": [],
                 "dependencies": [{"name": "core", "req": "*", "kind": null}]}
              ]
            }
        "#});
        assert_eq!(found, ["core"]);
    }

    #[test]
    fn versioned_dev_dependencies_both_ways_are_a_cycle() {
        let metadata: Metadata = serde_json::from_str(indoc! {r#"
            {
              "workspace_members": ["a", "b"],
              "packages": [
                {"id": "a", "name": "a", "version": "0.1.0", "manifest_path": "/w/a/Cargo.toml", "publish": null,
                 "dependencies": [{"name": "b", "req": "^0.1", "kind": "dev"}]},
                {"id": "b", "name": "b", "version": "0.1.0", "manifest_path": "/w/b/Cargo.toml", "publish": null,
                 "dependencies": [{"name": "a", "req": "^0.1", "kind": "dev"}]}
              ]
            }
        "#})
        .unwrap();
        let error = in_publish_order(&metadata).unwrap_err();
        assert!(format!("{error:#}").contains("a -> b -> a"), "{error:#}");
    }

    #[test]
    fn only_library_like_targets_have_an_api() {
        let metadata: TargetMetadata = serde_json::from_str(indoc! {r#"
            {
              "packages": [
                {"name": "cli", "targets": [{"kind": ["bin"]}, {"kind": ["test"]}]},
                {"name": "both", "targets": [{"kind": ["lib"]}, {"kind": ["bin"]}]},
                {"name": "typed", "targets": [{"kind": ["rlib", "cdylib"]}]},
                {"name": "derive", "targets": [{"kind": ["proc-macro"]}]}
              ]
            }
        "#})
        .unwrap();
        assert!(!library_in(&metadata, "cli").unwrap());
        assert!(library_in(&metadata, "both").unwrap());
        assert!(library_in(&metadata, "typed").unwrap());
        assert!(!library_in(&metadata, "derive").unwrap());
        let error = library_in(&metadata, "missing").unwrap_err();
        assert!(
            format!("{error:#}").contains("no package missing"),
            "{error:#}"
        );
    }

    /// The real `cargo metadata` resolves member globs, inherited versions,
    /// and `publish = false`.
    #[test]
    fn cargo_metadata_lists_the_publishing_members() {
        let root = TempDir::new().unwrap();
        let write = |path: &str, body: &str| {
            let path = root.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        };
        write(
            "Cargo.toml",
            indoc! {r#"
                [workspace]
                resolver = "3"
                members = ["crates/*"]

                [workspace.package]
                version = "0.0.2"
                edition = "2024"
            "#},
        );
        write(
            "crates/app/Cargo.toml",
            indoc! {r#"
                [package]
                name = "demo-app"
                version.workspace = true
                edition.workspace = true

                [dependencies]
                demo-core = { version = "=0.0.2", path = "../core" }
            "#},
        );
        write("crates/app/src/lib.rs", "");
        write(
            "crates/core/Cargo.toml",
            indoc! {r#"
                [package]
                name = "demo-core"
                version.workspace = true
                edition.workspace = true
            "#},
        );
        write("crates/core/src/lib.rs", "");
        write(
            "crates/xtask/Cargo.toml",
            indoc! {r#"
                [package]
                name = "xtask"
                version = "0.1.0"
                edition.workspace = true
                publish = false
            "#},
        );
        write("crates/xtask/src/main.rs", "fn main() {}");
        let manifest = root.path().join("Cargo.toml");
        assert!(is_virtual(&manifest).unwrap());
        assert!(!is_virtual(&root.path().join("crates/app/Cargo.toml")).unwrap());
        let found = members(&manifest).unwrap();
        let shown: Vec<(&str, &str)> = found
            .iter()
            .map(|member| (member.name.as_str(), member.version.as_str()))
            .collect();
        assert_eq!(shown, [("demo-core", "0.0.2"), ("demo-app", "0.0.2")]);
        assert!(has_library(&manifest, "demo-core").unwrap());
        assert!(!has_library(&root.path().join("crates/app/Cargo.toml"), "xtask").unwrap());
    }
}
