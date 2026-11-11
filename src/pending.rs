//! Which version commits still need a release.
//!
//! A release is complete when the registry lists each package's version, and
//! its tag, its Release, and every declared asset exist.
//!
//! `status` lists every version commit with an incomplete release, oldest
//! first. The workflow can then release each at its own commit. `release`
//! releases only the packages whose version commit is HEAD.
//!
//! A version an earlier commit set and no release has is listed at the commit
//! that set it. That happens when a later Version PR merged first.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::Path;

use anyhow::{Context, Result};
use ctl_core::input::Input;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::assets::{self, MatrixTarget};
use crate::config::Config;
use crate::driver::Driver;
use crate::git::{self, VersionCommit};
use crate::github::{self, ReleaseState};
use crate::publish::{self, PublishEntry};
use crate::registry::{self, RegistryCheck};
use crate::registry_token::RegistryToken;
use crate::{bump, publisher};

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
    /// The version its version commit sets: the one at HEAD, or an earlier
    /// one that was never released.
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
    /// The release tag the tarball is uploaded to.
    pub tag: String,
}

/// One package's state, before grouping by commit.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageState {
    package: PendingPackage,
    commit: VersionCommit,
    complete: bool,
    missing_assets: Vec<MatrixTarget>,
    /// The tag the missing assets upload to.
    assets_tag: String,
}

/// Find every version commit with an incomplete release.
///
/// A package at `0.0.0` is left out before any lookup: it has never been
/// released, and its first release is the `0.0.1` a Version PR prepares (see
/// [`bump::unreleased`]).
///
/// A package whose release is incomplete also lists each earlier version on
/// HEAD's first-parent line since its last tag. Each is listed at the commit
/// that set it, with every declared asset missing (see
/// [`git::unreleased_below`]).
///
/// With `skip_assets`, declared assets do not count toward completeness, so a
/// repository that turns assets off still reaches nothing to release.
///
/// A config with no packages, such as one that only vendors, has nothing to
/// release, so `status` works there without a lookup.
///
/// A registry `query` runs with `registry_token` when one is given, as in
/// `release`, because a private registry needs the same credential to answer.
/// `status` holds none.
///
/// # Errors
///
/// Returns an error when:
///
/// - a package's version, driver, or publisher cannot be resolved;
/// - its version commit cannot be found (see [`git::version_commit`]);
/// - the repository has no GitHub `origin`;
/// - a registry or GitHub lookup fails.
///
/// A lookup that cannot run is a [`crate::lookup::Unavailable`]. It covers a
/// missing GitHub repository, no answer from GitHub or a registry, and a walk
/// that reaches a shallow clone's boundary.
///
/// It also returns an error when a package's tag names the wrong commit. The
/// right commit is its version commit, or another commit HEAD contains at the
/// same version (see [`git::tagged_commit`]). Counting a release tagged in the
/// wrong place as complete would hide it.
pub fn pending(
    config: &Config,
    config_path: &Path,
    root: &Path,
    token: &str,
    skip_assets: bool,
    registry_token: Option<&RegistryToken>,
) -> Result<Pending> {
    if config.packages.is_empty() {
        return Ok(Pending::default());
    }
    let planned = publish::plan(config, root)?;
    let repo = github::repo(root)?;
    let (targets, assets_tag) = if skip_assets || config.assets.is_none() {
        (Vec::new(), String::new())
    } else {
        let plan = assets::plan(config, root)?;
        (plan.matrix.include, plan.tag)
    };
    let mut found = Vec::new();
    for (index, (spec, entry)) in config.packages.iter().zip(&planned.packages).enumerate() {
        if bump::unreleased(&entry.version) {
            continue;
        }
        let driver = spec.resolve(config, root)?;
        let commit = git::version_commit(root, &root.join(&spec.path), &|raw: &str| {
            version_in(&driver, raw)
        })?;
        found.push((
            (index, spec, driver),
            entry,
            tag_of(&planned, entry)?,
            commit,
        ));
    }
    let release_commits = newest_per_tag(&found);
    let env = registry_token.map(RegistryToken::env);
    let env = env.as_slice();
    let mut states = Vec::new();
    let mut releases: BTreeMap<String, ReleaseState> = BTreeMap::new();
    for ((index, spec, driver), entry, tag, _) in &found {
        let commit = release_commits[tag.as_str()].clone();
        let release = match releases.entry(tag.clone()) {
            Entry::Occupied(held) => held.into_mut(),
            Entry::Vacant(slot) => {
                let state = github::release_state(token, &repo, slot.key())?;
                slot.insert(state)
            }
        };
        let read = |raw: &str| version_in(driver, raw);
        let commit = match &release.tag {
            Some(named) if !named.eq_ignore_ascii_case(&commit.sha) => {
                git::tagged_commit(root, &root.join(&spec.path), named, &read)?.with_context(
                    || {
                        format!(
                            "tag {tag} names {named}: not the version commit {} and not a \
                             first-parent commit at {}@{}",
                            commit.sha, entry.name, entry.version
                        )
                    },
                )?
            }
            Some(_) | None => commit,
        };
        let listed = registry_lists(config, root, spec, entry, release, env)?;
        let head_targets: &[MatrixTarget] = if *index == 0 { &targets } else { &[] };
        let declared = owed(head_targets, &release.assets, || {
            declared_at(root, config_path, &commit.sha)
        })?;
        let state = state_of(entry, commit, listed, release, &declared, &assets_tag);
        let incomplete = !state.complete;
        states.push(state);
        if incomplete {
            let name = config.tags.uses_name().then_some(entry.name.as_str());
            let earlier =
                git::unreleased_below(root, &root.join(&spec.path), &read, &mut |version| {
                    let checks = checks_at(config, root, spec, entry, version)?;
                    version_released(&checks, version, env, || {
                        let tag = config.tags.render(version, name)?;
                        Ok(github::release_state(token, &repo, &tag)?.tag.is_some())
                    })
                })?;
            for skipped in earlier {
                let missing_assets = owed(head_targets, &[], || {
                    declared_at(root, config_path, &skipped.sha)
                })?;
                states.push(PackageState {
                    assets_tag: config.tags.render(&skipped.version, name)?,
                    package: PendingPackage {
                        name: entry.name.clone(),
                        version: skipped.version,
                    },
                    commit: VersionCommit {
                        sha: skipped.sha,
                        depth: skipped.depth,
                    },
                    complete: false,
                    missing_assets,
                });
            }
        }
    }
    Ok(group(states))
}

/// The asset targets a release owes: those in `targets`, which HEAD declares,
/// that its version commit declared too. A target added after the release
/// never reopens it.
///
/// A release that already holds every target's asset owes them all, and
/// `declared` is never called, so its old config never has to parse.
/// Otherwise `declared` gives the target ids the version commit's config
/// declared, and `None` for a commit with no config, which owes nothing.
fn owed(
    targets: &[MatrixTarget],
    assets: &[String],
    declared: impl FnOnce() -> Result<Option<Vec<String>>>,
) -> Result<Vec<MatrixTarget>> {
    if targets.iter().all(|target| assets.contains(&target.asset)) {
        return Ok(targets.to_vec());
    }
    let ids = declared()?.unwrap_or_default();
    Ok(targets
        .iter()
        .filter(|target| ids.contains(&target.id))
        .cloned()
        .collect())
}

/// The asset target ids the config at `path` declared at commit `sha`, or
/// `None` when that commit has no file there. `path` is the config this run
/// loaded, so a nested or renamed config is read where it lives.
fn declared_at(root: &Path, path: &Path, sha: &str) -> Result<Option<Vec<String>>> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve {}", path.display()))?;
    let Some(raw) = git::file_at(root, sha, &path)? else {
        return Ok(None);
    };
    let declared: DeclaredTargets = Input::new(format!("{} at {sha}", path.display()), &raw)
        .parse()
        .with_context(|| format!("the asset targets of {} at commit {sha}", path.display()))?;
    Ok(Some(
        declared
            .assets
            .map(|assets| assets.targets.into_keys().collect())
            .unwrap_or_default(),
    ))
}

/// The asset target ids an old revision of the config holds, and nothing
/// else. It is read without the current schema: a key that a later verctl
/// renamed or removed must not stop an old release from being judged. Unknown
/// keys are ignored here and only here.
#[derive(Deserialize)]
struct DeclaredTargets {
    /// `assets`, when the revision declares any.
    #[serde(default)]
    assets: Option<TargetIds>,
}

/// The `assets` mapping of an old config, reduced to its target ids.
#[derive(Deserialize)]
struct TargetIds {
    /// Each target, by id. Its fields are not read.
    #[serde(default)]
    targets: BTreeMap<String, IgnoredAny>,
}

/// The version `driver` reads from one revision of a manifest, or `None` when
/// that revision holds none.
fn version_in(driver: &Driver, raw: &str) -> Option<String> {
    driver
        .read(raw)
        .ok()
        .map(|version| version.trim().to_owned())
}

/// The commit that releases each tag: the newest version commit among the
/// packages under it. A shared `v{version}` tag can cover packages whose
/// manifests reached that version in different commits. Only the newest of
/// those has every one of them at the tagged version, so the release and its
/// tag belong there.
fn newest_per_tag<S, E>(found: &[(S, E, String, VersionCommit)]) -> BTreeMap<&str, VersionCommit> {
    let mut newest: BTreeMap<&str, VersionCommit> = BTreeMap::new();
    for (_, _, tag, commit) in found {
        newest
            .entry(tag.as_str())
            .and_modify(|held| {
                if commit.depth < held.depth {
                    *held = commit.clone();
                }
            })
            .or_insert_with(|| commit.clone());
    }
    newest
}

/// The tag `release` would create for `entry`.
fn tag_of(planned: &publish::PublishPlan, entry: &PublishEntry) -> Result<String> {
    planned
        .tags
        .iter()
        .find(|tag| tag.packages.iter().any(|named| named.name == entry.name))
        .map(|tag| tag.tag.clone())
        .with_context(|| format!("no release tag for {}", entry.name))
}

/// Whether the registry has `entry`'s version: for a workspace package, every
/// member's.
///
/// A package with no registry, or an upload with no check, counts as listed
/// once its tag exists. `release` creates the tag of a package with an
/// unchecked upload only after the upload, so the tag is the record. A tag
/// with only checked uploads is created first and records nothing.
///
/// A `query` runs with `env`, as [`RegistryCheck::lists`] describes.
fn registry_lists(
    config: &Config,
    root: &Path,
    spec: &crate::config::PackageSpec,
    entry: &PublishEntry,
    release: &ReleaseState,
    env: &[(&str, &str)],
) -> Result<bool> {
    if entry.argv.is_empty() {
        return Ok(true);
    }
    for upload in publisher::uploads(config, spec, root, entry)? {
        let listed = match &upload.check {
            RegistryCheck::Unchecked => release.tag.is_some(),
            check @ (RegistryCheck::CratesIo { .. } | RegistryCheck::Query { .. }) => {
                check.lists(&upload.version, env)?
            }
        };
        if !listed {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The registry checks that say whether `version` of a package is published.
///
/// Any package but a workspace has its single check built for `version`,
/// since a `query` carries the version it was built for.
/// A workspace package checks the members [`members_at`] keeps.
fn checks_at(
    config: &Config,
    root: &Path,
    spec: &crate::config::PackageSpec,
    entry: &PublishEntry,
    version: &str,
) -> Result<Vec<RegistryCheck>> {
    if entry.argv.is_empty() {
        return Ok(Vec::new());
    }
    if publisher::workspace_members(config, spec, root)?.is_some() {
        let wanted =
            semver::Version::parse(version).with_context(|| format!("semver {version:?}"))?;
        let uploads = publisher::uploads(config, spec, root, entry)?;
        return members_at(uploads, &entry.version, |name| {
            Ok(registry::crates_io_baseline(registry::CRATES_IO_INDEX, name, &wanted)?.is_some())
        });
    }
    Ok(vec![publisher::registry_check(
        config, spec, root, version,
    )?])
}

/// The checks of the workspace members that took part in an earlier
/// release.
///
/// The members are read at HEAD, so two of them can say nothing about that
/// release:
///
/// - a member whose version is not the package's moves on its own;
/// - a crates.io member that `existed` says has no version at or below the
///   earlier one was added since.
///
/// When no member is left, [`version_released`] falls back to the tag.
fn members_at(
    uploads: Vec<publisher::Upload>,
    package_version: &str,
    existed: impl Fn(&str) -> Result<bool>,
) -> Result<Vec<RegistryCheck>> {
    let mut checks = Vec::new();
    for upload in uploads {
        if upload.version != package_version {
            continue;
        }
        if let RegistryCheck::CratesIo { name } = &upload.check
            && !existed(name)?
        {
            continue;
        }
        checks.push(upload.check);
    }
    Ok(checks)
}

/// Whether an earlier `version` of a package counts as released.
///
/// When every upload has a registry check, the registry decides.
/// `release` creates that package's tag before the upload, so a failed upload
/// leaves a tag for a version nobody published.
/// A package with an unchecked upload, or with none, is released once its tag
/// exists, because `release` creates that tag after the upload.
/// `tag_exists` runs only then.
fn version_released(
    checks: &[RegistryCheck],
    version: &str,
    env: &[(&str, &str)],
    tag_exists: impl FnOnce() -> Result<bool>,
) -> Result<bool> {
    if checks.is_empty()
        || checks
            .iter()
            .any(|check| matches!(check, RegistryCheck::Unchecked))
    {
        return tag_exists();
    }
    for check in checks {
        if !check.lists(version, env)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn state_of(
    entry: &PublishEntry,
    commit: VersionCommit,
    listed: bool,
    release: &ReleaseState,
    declared: &[MatrixTarget],
    assets_tag: &str,
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
        assets_tag: assets_tag.to_owned(),
    }
}

/// The incomplete packages at one version commit, while grouping.
struct AtCommit {
    depth: usize,
    packages: Vec<PendingPackage>,
    missing: Vec<(MatrixTarget, String)>,
}

/// Group incomplete packages by version commit, oldest commit first. Assets
/// belong to the first package, so each asset job uploads to the tag of that
/// package's version at its commit.
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
        let tag = state.assets_tag;
        slot.missing.extend(
            state
                .missing_assets
                .into_iter()
                .map(|target| (target, tag.clone())),
        );
    }
    let mut ordered: Vec<(String, AtCommit)> = by_commit.into_iter().collect();
    ordered.sort_by_key(|(_, at)| std::cmp::Reverse(at.depth));
    let mut pending = Pending::default();
    for (sha, at) in ordered {
        pending
            .assets
            .extend(at.missing.into_iter().map(|(target, tag)| PendingAsset {
                commit: sha.clone(),
                id: target.id,
                labels: target.labels,
                tag,
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
        DeclaredTargets, PackageState, Pending, PendingAsset, PendingPackage, group, members_at,
        newest_per_tag, owed, state_of, version_released, write_github_output,
    };
    use crate::assets::MatrixTarget;
    use crate::git::VersionCommit;
    use crate::github::ReleaseState;
    use crate::publish::PublishEntry;
    use crate::registry::RegistryCheck;

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
            "v0.4.6",
        )
    }

    #[test]
    fn a_shared_tag_is_released_at_its_newest_version_commit() {
        let found = vec![
            ((), (), "v1.0.0".to_owned(), at("old", 3)),
            ((), (), "v1.0.0".to_owned(), at("new", 1)),
            ((), (), "app@0.2.1".to_owned(), at("app", 2)),
        ];
        let newest = newest_per_tag(&found);
        assert_eq!(newest["v1.0.0"], at("new", 1));
        assert_eq!(newest["app@0.2.1"], at("app", 2));
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

    /// A version an earlier commit set and no release has is planned at that
    /// commit, before HEAD's. Its assets upload to its own tag.
    #[test]
    fn a_skipped_version_is_planned_first_with_its_own_tag() {
        let declared = [target("linux-x64")];
        let head = state_of(
            &entry("demo", "0.4.7"),
            at("bbb", 0),
            false,
            &ReleaseState::default(),
            &declared,
            "v0.4.7",
        );
        let skipped = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 1),
            false,
            &ReleaseState::default(),
            &declared,
            "v0.4.6",
        );
        let pending = group(vec![head, skipped]);
        let shas: Vec<&str> = pending.commits.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(shas, ["aaa", "bbb"]);
        let tags: Vec<(&str, &str)> = pending
            .assets
            .iter()
            .map(|asset| (asset.commit.as_str(), asset.tag.as_str()))
            .collect();
        assert_eq!(tags, [("aaa", "v0.4.6"), ("bbb", "v0.4.7")]);
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
            "v0.4.6",
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
            "v0.4.6",
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
            "v0.4.6",
        );
        let pending = group(vec![state]);
        assert_eq!(pending.commits.len(), 1);
        assert_eq!(
            pending.assets,
            [PendingAsset {
                commit: "aaa".into(),
                id: "darwin-arm64".into(),
                labels: vec!["darwin-arm64-runner".into()],
                tag: "v0.4.6".into(),
            }]
        );
    }

    fn ids(targets: &[MatrixTarget]) -> Vec<&str> {
        targets.iter().map(|target| target.id.as_str()).collect()
    }

    /// A target added after a release leaves that release complete: its
    /// version commit declared only `linux-x64`, and the Release holds it.
    #[test]
    fn a_target_added_after_a_release_does_not_reopen_it() {
        let targets = [target("linux-x64"), target("darwin-arm64")];
        let held = ["demo-linux-x64.tar.gz".to_owned()];
        let owed = owed(&targets, &held, || Ok(Some(vec!["linux-x64".into()]))).unwrap();
        assert_eq!(ids(&owed), ["linux-x64"]);
        let release = finished(&["demo-linux-x64.tar.gz"]);
        let state = state_of(
            &entry("demo", "0.0.1"),
            at("aaa", 1),
            true,
            &release,
            &owed,
            "v0.0.1",
        );
        assert!(state.complete);
    }

    #[test]
    fn a_target_the_release_commit_declared_is_still_owed() {
        let targets = [target("linux-x64"), target("darwin-arm64")];
        let held = ["demo-linux-x64.tar.gz".to_owned()];
        let declared = || Ok(Some(vec!["linux-x64".into(), "darwin-arm64".into()]));
        assert_eq!(
            ids(&owed(&targets, &held, declared).unwrap()),
            ["linux-x64", "darwin-arm64"]
        );
    }

    /// An old revision may hold keys the current schema refuses. Only its
    /// target ids are read, so those keys do not matter.
    #[test]
    fn an_old_config_names_its_targets_whatever_else_it_holds() {
        let declared: DeclaredTargets = ctl_core::input::Input::new(
            "old.yaml",
            indoc::indoc! {"
                drivers: {}
                a_key_removed_later: true
                assets:
                  bin: demo
                  targets:
                    linux-x64: {}
                    darwin-arm64:
                      runner: arm64
            "},
        )
        .parse()
        .unwrap();
        let ids: Vec<String> = declared.assets.unwrap().targets.into_keys().collect();
        assert_eq!(ids, ["darwin-arm64", "linux-x64"]);
    }

    #[test]
    fn a_commit_with_no_config_owes_no_asset() {
        let targets = [target("linux-x64")];
        assert_eq!(
            ids(&owed(&targets, &[], || Ok(None)).unwrap()),
            Vec::<&str>::new()
        );
    }

    /// The old config is read only for a release that lacks an asset, so a
    /// complete release never needs it to parse.
    #[test]
    fn a_complete_release_never_reads_its_old_config() {
        let targets = [target("linux-x64")];
        let held = ["demo-linux-x64.tar.gz".to_owned()];
        let owed = owed(&targets, &held, || {
            panic!("the old config was read for a complete release")
        })
        .unwrap();
        assert_eq!(ids(&owed), ["linux-x64"]);
    }

    #[test]
    fn a_registry_that_lacks_the_version_is_incomplete() {
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            false,
            &finished(&[]),
            &[],
            "v0.4.6",
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

    fn query(argv: &[&str]) -> RegistryCheck {
        RegistryCheck::Query {
            argv: argv.iter().map(|word| (*word).to_owned()).collect(),
            dir: std::env::temp_dir(),
        }
    }

    /// A tag that exists says nothing about a package whose tag `release`
    /// creates before the upload: a failed `cargo publish` leaves it behind.
    #[test]
    fn an_earlier_version_with_checked_uploads_is_released_when_the_registry_lists_it() {
        let listed = version_released(&[query(&["echo", "1.0.0"])], "1.0.0", &[], || {
            panic!("the registry decides, so the tag is not read")
        })
        .unwrap();
        assert!(listed);
        let unlisted = version_released(&[query(&["false"])], "1.0.0", &[], || Ok(true)).unwrap();
        assert!(!unlisted);
        let one_of_two = version_released(
            &[query(&["echo", "1.0.0"]), query(&["false"])],
            "1.0.0",
            &[],
            || Ok(true),
        )
        .unwrap();
        assert!(!one_of_two);
    }

    #[test]
    fn an_earlier_version_with_an_unchecked_upload_or_none_is_released_with_its_tag() {
        for checks in [
            vec![],
            vec![RegistryCheck::Unchecked],
            vec![query(&["echo", "1.0.0"]), RegistryCheck::Unchecked],
        ] {
            assert!(version_released(&checks, "1.0.0", &[], || Ok(true)).unwrap());
            assert!(!version_released(&checks, "1.0.0", &[], || Ok(false)).unwrap());
        }
    }

    fn member(name: &str, version: &str) -> crate::publisher::Upload {
        crate::publisher::Upload {
            name: name.to_owned(),
            version: version.to_owned(),
            manifest: std::path::PathBuf::from(format!("{name}/Cargo.toml")),
            argv: vec!["cargo".to_owned(), "publish".to_owned()],
            check: RegistryCheck::CratesIo {
                name: name.to_owned(),
            },
        }
    }

    /// A member added since the earlier release, and one that moves on its
    /// own, say nothing about that release, so only the others are asked.
    #[test]
    fn an_earlier_workspace_release_asks_only_the_members_it_had() {
        let checks = members_at(
            vec![
                member("core", "0.7.0"),
                member("added", "0.7.0"),
                member("own", "1.4.0"),
            ],
            "0.7.0",
            |name| Ok(name != "added"),
        )
        .unwrap();
        assert_eq!(
            checks,
            vec![RegistryCheck::CratesIo {
                name: "core".to_owned()
            }]
        );
        let none = members_at(vec![member("added", "0.7.0")], "0.7.0", |_| Ok(false)).unwrap();
        assert_eq!(none, Vec::<RegistryCheck>::new());
    }
}
