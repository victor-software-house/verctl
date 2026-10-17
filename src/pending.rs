//! Which version commits still need a release.
//!
//! A release is complete when the registry lists each package's version and
//! its tag, its Release, and every declared asset exist. `publish --plan` lists
//! every version commit with an incomplete release, oldest first, so the
//! workflow can release each at its own commit; `publish` releases only the
//! packages whose version commit is HEAD.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::assets::{self, MatrixTarget};
use crate::config::Config;
use crate::git::{self, VersionCommit};
use crate::github::{self, ReleaseState, Repo};
use crate::publish::{self, PublishEntry};
use crate::publisher;
use crate::registry::RegistryCheck;

/// Every version commit with an incomplete release, and the assets missing at
/// each.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Pending {
    /// Version commits, oldest first.
    pub commits: Vec<PendingCommit>,
    /// Asset jobs to run: one per version commit and missing target.
    pub assets: Vec<PendingAsset>,
}

impl Pending {
    /// Whether nothing is left to release.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commits.is_empty()
    }

    /// The pending commit at `sha`, if any.
    #[must_use]
    pub fn at(&self, sha: &str) -> Option<&PendingCommit> {
        self.commits
            .iter()
            .find(|commit| commit.sha.eq_ignore_ascii_case(sha))
    }
}

/// One version commit and the packages it still has to release.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PendingCommit {
    /// Full hex SHA of the commit.
    pub sha: String,
    /// The incomplete packages whose version commit this is.
    pub packages: Vec<PendingPackage>,
}

/// A package whose release is incomplete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PendingPackage {
    /// Package name.
    pub name: String,
    /// Version at HEAD.
    pub version: String,
}

/// One asset job: a target whose tarball the Release at `commit` lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PendingAsset {
    /// The version commit the job checks out.
    pub commit: String,
    /// The target id, such as `darwin-arm64`.
    pub id: String,
    /// The machine labels the job runs on.
    pub labels: Vec<String>,
}

/// One package's state, before grouping by commit.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageState {
    package: PendingPackage,
    commit: VersionCommit,
    complete: bool,
    missing_assets: Vec<MatrixTarget>,
}

/// Find every version commit with an incomplete release.
///
/// With `skip_assets`, declared assets do not count toward completeness, so a
/// repository that turns assets off still reaches nothing to release.
///
/// # Errors
///
/// Returns an error when a package's version, driver, or publisher cannot be
/// resolved, its version commit cannot be found (see
/// [`git::version_commit`]), the repository has no GitHub `origin`, or a
/// registry or GitHub lookup fails.
pub fn pending(config: &Config, root: &Path, token: &str, skip_assets: bool) -> Result<Pending> {
    let planned = publish::plan(config, root)?;
    let repo = github::repo(root)?;
    let targets = if skip_assets || config.assets.is_none() {
        Vec::new()
    } else {
        assets::plan(config, root)?.matrix.include
    };
    let mut states = Vec::new();
    let mut releases: BTreeMap<String, ReleaseState> = BTreeMap::new();
    for (index, (spec, entry)) in config.packages.iter().zip(&planned.packages).enumerate() {
        let driver = spec.resolve(config, root)?;
        let read = |raw: &str| {
            driver
                .read(raw)
                .ok()
                .map(|version| version.trim().to_owned())
        };
        let commit = git::version_commit(root, &root.join(&spec.path), &read)?;
        let release = match releases.entry(tag_of(&planned, entry)?) {
            Entry::Occupied(found) => found.into_mut(),
            Entry::Vacant(slot) => {
                let state = github::release_state(token, &repo, slot.key())?;
                slot.insert(state)
            }
        };
        let listed = registry_lists(config, root, spec, entry, release)?;
        let declared: &[MatrixTarget] = if index == 0 { &targets } else { &[] };
        states.push(state_of(entry, commit, listed, release, declared));
    }
    Ok(group(states))
}

/// The tag `publish` would create for `entry`.
fn tag_of(planned: &publish::PublishPlan, entry: &PublishEntry) -> Result<String> {
    planned
        .tags
        .iter()
        .find(|tag| tag.packages.iter().any(|named| named.name == entry.name))
        .map(|tag| tag.tag.clone())
        .with_context(|| format!("no release tag for {}", entry.name))
}

/// Whether the registry has `entry`'s version. A package with no registry, or
/// with no check, counts as listed once its tag exists: `publish` creates the
/// tag only after the upload, so the tag is the record.
fn registry_lists(
    config: &Config,
    root: &Path,
    spec: &crate::config::PackageSpec,
    entry: &PublishEntry,
    release: &ReleaseState,
) -> Result<bool> {
    if entry.argv.is_empty() {
        return Ok(true);
    }
    match publisher::registry_check(config, spec, root, &entry.version)? {
        RegistryCheck::Unchecked => Ok(release.tag.is_some()),
        check @ (RegistryCheck::CratesIo { .. } | RegistryCheck::Query { .. }) => {
            check.lists(&entry.version)
        }
    }
}

fn state_of(
    entry: &PublishEntry,
    commit: VersionCommit,
    listed: bool,
    release: &ReleaseState,
    declared: &[MatrixTarget],
) -> PackageState {
    let missing_assets: Vec<MatrixTarget> = declared
        .iter()
        .filter(|target| !release.assets.contains(&target.asset))
        .cloned()
        .collect();
    PackageState {
        package: PendingPackage {
            name: entry.name.clone(),
            version: entry.version.clone(),
        },
        commit,
        complete: listed && release.tag.is_some() && release.release && missing_assets.is_empty(),
        missing_assets,
    }
}

/// The incomplete packages at one version commit, while grouping.
struct AtCommit {
    depth: usize,
    packages: Vec<PendingPackage>,
    missing: Vec<MatrixTarget>,
}

/// Group incomplete packages by version commit, oldest commit first.
fn group(states: Vec<PackageState>) -> Pending {
    let mut by_commit: BTreeMap<String, AtCommit> = BTreeMap::new();
    for state in states.into_iter().filter(|state| !state.complete) {
        let slot = by_commit
            .entry(state.commit.sha)
            .or_insert_with(|| AtCommit {
                depth: state.commit.depth,
                packages: Vec::new(),
                missing: Vec::new(),
            });
        slot.packages.push(state.package);
        slot.missing.extend(state.missing_assets);
    }
    let mut ordered: Vec<(String, AtCommit)> = by_commit.into_iter().collect();
    ordered.sort_by_key(|(_, at)| std::cmp::Reverse(at.depth));
    let mut pending = Pending::default();
    for (sha, at) in ordered {
        pending
            .assets
            .extend(at.missing.into_iter().map(|target| PendingAsset {
                commit: sha.clone(),
                id: target.id,
                labels: target.labels,
            }));
        pending.commits.push(PendingCommit {
            sha,
            packages: at.packages,
        });
    }
    pending
}

#[derive(Serialize)]
struct AssetMatrix<'a> {
    include: &'a [PendingAsset],
}

/// Append `commits` (a JSON list, oldest first), `pending`, and `assets` (a
/// job matrix of commit, target id, and labels) to the step output file
/// `path`.
///
/// # Errors
///
/// Returns an error when the output cannot be encoded or written.
pub fn write_github_output(pending: &Pending, path: &Path) -> Result<()> {
    let commits: Vec<&str> = pending
        .commits
        .iter()
        .map(|commit| commit.sha.as_str())
        .collect();
    let commits = serde_json::to_string(&commits).context("encode commits")?;
    let assets = serde_json::to_string(&AssetMatrix {
        include: &pending.assets,
    })
    .context("encode asset matrix")?;
    let body = ctl_core::formatdoc! {"
        commits={commits}
        pending={pending}
        assets={assets}
        ",
        pending = !pending.is_empty(),
    };
    github::write_output(path, &body)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::{
        PackageState, Pending, PendingAsset, PendingPackage, group, state_of, write_github_output,
    };
    use crate::assets::MatrixTarget;
    use crate::git::VersionCommit;
    use crate::github::ReleaseState;
    use crate::publish::PublishEntry;

    fn entry(name: &str, version: &str) -> PublishEntry {
        PublishEntry {
            name: name.into(),
            version: version.into(),
            noun: "crate".into(),
            label: "cargo".into(),
            argv: vec!["cargo".into(), "publish".into()],
        }
    }

    fn at(sha: &str, depth: usize) -> VersionCommit {
        VersionCommit {
            sha: sha.into(),
            depth,
        }
    }

    fn target(id: &str) -> MatrixTarget {
        MatrixTarget {
            id: id.into(),
            machine: id.into(),
            labels: vec![format!("{id}-runner")],
            os: "os".into(),
            arch: "arch".into(),
            triple: "triple".into(),
            asset: format!("demo-{id}.tar.gz"),
        }
    }

    fn finished(assets: &[&str]) -> ReleaseState {
        ReleaseState {
            tag: Some("a".repeat(40)),
            release: true,
            assets: assets.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    fn incomplete(name: &str, version: &str, commit: VersionCommit) -> PackageState {
        state_of(
            &entry(name, version),
            commit,
            false,
            &ReleaseState::default(),
            &[],
        )
    }

    #[test]
    fn every_incomplete_commit_oldest_first() {
        let pending = group(vec![
            incomplete("app", "0.2.1", at("ccc", 0)),
            incomplete("core", "0.3.1", at("aaa", 2)),
        ]);
        let shas: Vec<&str> = pending.commits.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(shas, ["aaa", "ccc"]);
        assert_eq!(
            pending.at("ccc").unwrap().packages,
            [PendingPackage {
                name: "app".into(),
                version: "0.2.1".into()
            }]
        );
    }

    #[test]
    fn a_push_after_a_finished_release_is_nothing_to_release() {
        let declared = [target("darwin-arm64"), target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            true,
            &finished(&["demo-darwin-arm64.tar.gz", "demo-linux-x64.tar.gz"]),
            &declared,
        );
        assert!(group(vec![state]).is_empty());
    }

    #[test]
    fn assets_turned_off_count_for_nothing() {
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            true,
            &finished(&[]),
            &[],
        );
        assert!(group(vec![state]).is_empty());
    }

    #[test]
    fn only_a_missing_asset_keeps_the_commit_pending() {
        let declared = [target("darwin-arm64"), target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 1),
            true,
            &finished(&["demo-linux-x64.tar.gz"]),
            &declared,
        );
        let pending = group(vec![state]);
        assert_eq!(pending.commits.len(), 1);
        assert_eq!(
            pending.assets,
            [PendingAsset {
                commit: "aaa".into(),
                id: "darwin-arm64".into(),
                labels: vec!["darwin-arm64-runner".into()],
            }]
        );
    }

    #[test]
    fn a_registry_that_lacks_the_version_is_incomplete() {
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            false,
            &finished(&[]),
            &[],
        );
        assert!(!group(vec![state]).is_empty());
    }

    #[test]
    fn the_plan_writes_commits_pending_and_assets() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("output");
        let pending = group(vec![incomplete("demo", "0.4.6", at("aaa", 1))]);
        write_github_output(&pending, &path).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                commits=["aaa"]
                pending=true
                assets={"include":[]}
            "#}
        );
        let nothing = dir.path().join("nothing");
        write_github_output(&Pending::default(), &nothing).unwrap();
        assert_eq!(
            std::fs::read_to_string(&nothing).unwrap(),
            indoc! {r#"
                commits=[]
                pending=false
                assets={"include":[]}
            "#}
        );
    }
}
