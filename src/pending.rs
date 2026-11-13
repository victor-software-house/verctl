//! Which version commits still need a release.
//!
//! A release is complete when the registry lists each package's version, and
//! its tag and its published Release exist, with every declared asset.
//! A Release with declared assets is a draft until every asset is uploaded.
//! A draft is incomplete, and `release` run again at its commit publishes it.
//! A published Release that lacks an asset is incomplete too, unless GitHub
//! holds it immutable.
//! An immutable Release takes no asset, so it counts complete with a
//! [`Shortfall`] to warn about.
//!
//! `status` lists every version commit with an incomplete release, oldest
//! first. The workflow can then release each at its own commit. `release`
//! releases only the packages whose version commit is HEAD.
//!
//! A version an earlier commit set and no release has is listed at the commit
//! that set it. That happens when a later Version PR merged first.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use ctl_core::input::Input;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::assets::{self, MatrixTarget};
use crate::config::{Assets, Config, PackageSpec};
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
    /// Published Releases that are immutable and lack declared assets. They
    /// count complete, and no asset job is planned for them.
    pub shortfalls: Vec<Shortfall>,
    /// Version commits whose releases are all done, so a run there has
    /// nothing left to do.
    pub finished: Vec<String>,
}

/// A published, immutable Release that lacks declared assets.
///
/// GitHub refuses a new asset on it and never lets it be unpublished, so only
/// a later version can ship what it lacks ([immutable releases]).
///
/// [immutable releases]: https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Shortfall {
    /// The release tag.
    pub tag: String,
    /// The file names of the declared assets it lacks.
    pub assets: Vec<String>,
}

impl fmt::Display for Shortfall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "release {tag} is published and immutable without {assets}; a later version must ship them",
            tag = self.tag,
            assets = self.assets.join(", ")
        )
    }
}

impl Pending {
    /// Whether nothing is left to release.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commits.is_empty()
    }

    /// Whether `sha` is a version commit whose releases are all done.
    #[must_use]
    pub fn is_finished(&self, sha: &str) -> bool {
        self.at(sha).is_none()
            && self
                .finished
                .iter()
                .any(|finished| finished.eq_ignore_ascii_case(sha))
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
    /// The file names of the assets this commit's Release is expected to
    /// hold, drafts and missing ones alike. The workflow hands them to each
    /// leg at this commit, which declares exactly these.
    pub declared: Vec<String>,
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
    /// Whether the registry already lists the version or, for an upload with
    /// no registry check, its tag exists. Only the Release or its assets are
    /// then left, such as a draft waiting to be published, and `release`
    /// runs no upload for it.
    #[serde(skip)]
    pub listed: bool,
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
    /// Set when the Release is published, immutable, and lacks declared
    /// assets: it is complete, and an asset job could not upload them.
    shortfall: Option<Shortfall>,
    /// The file names of every asset the Release is checked for.
    declared: Vec<String>,
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
/// A non-empty `declared_assets` keeps only the targets whose tarball it names,
/// as `release --declared-assets` does at a version commit.
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
    declared_assets: &[String],
    registry_token: Option<&RegistryToken>,
) -> Result<Pending> {
    if config.packages.is_empty() {
        return Ok(Pending::default());
    }
    let planned = publish::plan(config, root)?;
    let repo = github::repo(root)?;
    let (targets, assets_tag, bin) = if skip_assets || config.assets.is_none() {
        (Vec::new(), String::new(), String::new())
    } else {
        let plan = assets::plan(config, root)?;
        let include = plan
            .matrix
            .include
            .into_iter()
            .filter(|target| declared_assets.is_empty() || declared_assets.contains(&target.asset))
            .collect();
        (include, plan.tag, plan.bin)
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
        let declared = checked_targets(head_targets, &release.assets, || {
            declared_at(root, config_path, &commit.sha)
        })?;
        let state = state_of(entry, commit, listed, release, &declared, &assets_tag);
        let walk = !state.complete || !head_targets.is_empty();
        states.push(state);
        if walk {
            let versions = Versions {
                config,
                config_path,
                root,
                token,
                repo: &repo,
                env,
                targets: head_targets,
                bin: &bin,
            };
            states.extend(versions.earlier(spec, entry, &read)?);
        }
    }
    Ok(group(states))
}

/// What the earlier versions of a package are read against.
struct Versions<'a> {
    config: &'a Config,
    config_path: &'a Path,
    root: &'a Path,
    token: &'a str,
    repo: &'a github::Repo,
    /// The registry token's variable, when one is held.
    env: &'a [(&'a str, &'a str)],
    /// The targets HEAD declares for the package that carries assets, named for
    /// HEAD's version. Empty for any other package.
    targets: &'a [MatrixTarget],
    /// The binary name the tarball names start with.
    bin: &'a str,
}

/// What is known of an earlier version once probed: its Release, read only
/// when the repository declares assets, and whether the registry or its tag
/// already has the version.
type Probed = (Option<ReleaseState>, bool);

impl Versions<'_> {
    /// The state of each version an earlier commit set that no release
    /// finished, oldest commit last.
    ///
    /// A version counts as released when its registry lists it or its tag
    /// exists and, in a repository that declares assets, its Release is not a
    /// draft. A draft waits for its assets and its publication, so it is
    /// planned like any other incomplete release.
    fn earlier(
        &self,
        spec: &PackageSpec,
        entry: &PublishEntry,
        read: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Vec<PackageState>> {
        let name = self.config.tags.uses_name().then_some(entry.name.as_str());
        let mut probed: BTreeMap<String, Probed> = BTreeMap::new();
        let earlier = git::unreleased_below(
            self.root,
            &self.root.join(&spec.path),
            read,
            &mut |version| {
                let (held, listed) = self.probe(spec, entry, name, version)?;
                let released = listed && !held.as_ref().is_some_and(|state| state.draft);
                probed.insert(version.to_owned(), (held, listed));
                Ok(released)
            },
        )?;
        let mut states = Vec::new();
        for skipped in earlier {
            let probe = probed.remove(&skipped.version).unwrap_or_default();
            states.push(self.state(entry, name, skipped, probe)?);
        }
        Ok(states)
    }

    /// Ask the registry or the tag, and GitHub for the Release when assets are
    /// declared, whether `version` is already uploaded.
    fn probe(
        &self,
        spec: &PackageSpec,
        entry: &PublishEntry,
        name: Option<&str>,
        version: &str,
    ) -> Result<Probed> {
        let checks = checks_at(self.config, self.root, spec, entry, version)?;
        let tag = self.config.tags.render(version, name)?;
        let held = if self.targets.is_empty() {
            None
        } else {
            Some(github::release_state(self.token, self.repo, &tag)?)
        };
        let listed = version_released(&checks, version, self.env, || match &held {
            Some(state) => Ok(state.tag.is_some()),
            None => Ok(github::release_state(self.token, self.repo, &tag)?
                .tag
                .is_some()),
        })?;
        Ok((held, listed))
    }

    /// The state of `skipped`, from what GitHub holds for it.
    /// Its targets carry the tarball names of its own version, which its
    /// Release holds, not HEAD's.
    fn state(
        &self,
        entry: &PublishEntry,
        name: Option<&str>,
        skipped: git::Unreleased,
        (held, listed): Probed,
    ) -> Result<PackageState> {
        let held = held.unwrap_or_default();
        let tag = self.config.tags.render(&skipped.version, name)?;
        let named = at_version(self.targets, self.bin, &skipped.version);
        let declared = checked_targets(&named, &held.assets, || {
            declared_at(self.root, self.config_path, &skipped.sha)
        })?;
        let earlier = PublishEntry {
            version: skipped.version,
            ..entry.clone()
        };
        let commit = VersionCommit {
            sha: skipped.sha,
            depth: skipped.depth,
        };
        Ok(state_of(&earlier, commit, listed, &held, &declared, &tag))
    }
}

/// `targets` with the tarball names of `version`, which an earlier release
/// holds under its own version, not HEAD's.
fn at_version(targets: &[MatrixTarget], bin: &str, version: &str) -> Vec<MatrixTarget> {
    targets
        .iter()
        .map(|target| MatrixTarget {
            asset: assets::asset_name(bin, version, &target.os, &target.arch),
            ..target.clone()
        })
        .collect()
}

/// The asset targets a release is checked for. HEAD declares `targets`, and
/// the release is checked only for those its version commit declared too. A
/// target added after the release never reopens it.
///
/// A release that already holds every target's asset is checked for them all.
/// `declared` is then never called, so its old config never has to parse.
/// Otherwise `declared` gives the target ids the version commit's config
/// declared, and `None` for a commit with no config, which is checked for no
/// asset.
fn checked_targets(
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
    Ok(Some(declared.assets.map_or_default(|assets| assets.0)))
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

/// The `assets` mapping of an old config, reduced to its target ids, in file
/// order. A target sits directly under `assets`, beside the settings
/// ([`Assets::SETTINGS`]), so every key that is not a setting is a target.
/// The fields of a target are not read.
struct TargetIds(Vec<String>);

impl<'de> Deserialize<'de> for TargetIds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let keys = mapping_keys(deserializer)?;
        Ok(Self(
            keys.into_iter()
                .filter(|key| !Assets::SETTINGS.contains(&key.as_str()))
                .collect(),
        ))
    }
}

/// The keys of a mapping, each value skipped unread.
fn mapping_keys<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    struct Keys;
    impl<'de> Visitor<'de> for Keys {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a mapping of asset targets")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut keys = Vec::new();
            while let Some(key) = map.next_key::<String>()? {
                map.next_value::<IgnoredAny>()?;
                keys.push(key);
            }
            Ok(keys)
        }
    }
    deserializer.deserialize_map(Keys)
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
    spec: &PackageSpec,
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
    spec: &PackageSpec,
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

/// One package's state at its release commit.
///
/// A draft Release is incomplete even when it holds every declared asset: the
/// next `release` run publishes it. A published Release with every declared
/// asset is complete.
///
/// A published Release that lacks declared assets depends on whether GitHub
/// holds it immutable:
///
/// - Immutable, it can take no asset, so it is complete and no asset job is
///   planned. The state carries a [`Shortfall`] for `status` and `release` to
///   warn about.
/// - Mutable, it is incomplete, and only the missing targets are planned.
fn state_of(
    entry: &PublishEntry,
    commit: VersionCommit,
    listed: bool,
    release: &ReleaseState,
    declared: &[MatrixTarget],
    assets_tag: &str,
) -> PackageState {
    let missing: Vec<MatrixTarget> = declared
        .iter()
        .filter(|target| !release.assets.contains(&target.asset))
        .cloned()
        .collect();
    let published = release.release && !release.draft;
    let (missing_assets, shortfall) = if published && release.immutable && !missing.is_empty() {
        let shortfall = Shortfall {
            tag: assets_tag.to_owned(),
            assets: missing.into_iter().map(|target| target.asset).collect(),
        };
        (Vec::new(), Some(shortfall))
    } else {
        (missing, None)
    };
    PackageState {
        package: PendingPackage {
            name: entry.name.clone(),
            version: entry.version.clone(),
            listed,
        },
        commit,
        complete: listed && release.tag.is_some() && published && missing_assets.is_empty(),
        missing_assets,
        assets_tag: assets_tag.to_owned(),
        shortfall,
        declared: declared.iter().map(|target| target.asset.clone()).collect(),
    }
}

/// The incomplete packages at one version commit, while grouping.
struct AtCommit {
    depth: usize,
    packages: Vec<PendingPackage>,
    missing: Vec<(MatrixTarget, String)>,
    declared: Vec<String>,
}

/// Group incomplete packages by version commit, oldest commit first. Assets
/// belong to the first package, so each asset job uploads to the tag of that
/// package's version at its commit.
fn group(states: Vec<PackageState>) -> Pending {
    let mut by_commit: BTreeMap<String, AtCommit> = BTreeMap::new();
    let mut shortfalls = Vec::new();
    let mut finished = Vec::new();
    for mut state in states {
        shortfalls.extend(state.shortfall.take());
        if state.complete {
            if !finished.contains(&state.commit.sha) {
                finished.push(state.commit.sha);
            }
            continue;
        }
        let slot = by_commit
            .entry(state.commit.sha)
            .or_insert_with(|| AtCommit {
                depth: state.commit.depth,
                packages: Vec::new(),
                missing: Vec::new(),
                declared: Vec::new(),
            });
        slot.packages.push(state.package);
        slot.declared.extend(state.declared);
        let tag = state.assets_tag;
        slot.missing.extend(
            state
                .missing_assets
                .into_iter()
                .map(|target| (target, tag.clone())),
        );
    }
    let mut ordered: Vec<(String, AtCommit)> = by_commit.into_iter().collect();
    ordered.sort_by_key(|(_, at)| Reverse(at.depth));
    let mut pending = Pending {
        shortfalls,
        finished,
        ..Pending::default()
    };
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
            declared: at.declared,
        });
    }
    pending
}

#[derive(Serialize)]
struct AssetMatrix<'a> {
    include: &'a [PendingAsset],
}

/// Append `commits` (a JSON list, oldest first), `pending`, `assets` (a job
/// matrix of commit, target id, and labels), and `declared` to the step output
/// file `path`.
/// `declared` maps each commit to the comma-separated file names of the assets
/// its Release is expected to hold, so a leg at that commit declares the same
/// set the plan scheduled.
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
    let declared: BTreeMap<&str, String> = pending
        .commits
        .iter()
        .map(|commit| (commit.sha.as_str(), commit.declared.join(",")))
        .collect();
    let declared = serde_json::to_string(&declared).context("encode declared assets")?;
    let body = ctl_core::formatdoc! {"
        commits={commits}
        pending={pending}
        assets={assets}
        declared={declared}
        ",
        pending = !pending.is_empty(),
    };
    github::write_output(path, &body)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::{env, fs};

    use ctl_core::input::Input;
    use indoc::indoc;

    use super::{
        DeclaredTargets, PackageState, Pending, PendingAsset, PendingPackage, Shortfall,
        checked_targets, group, members_at, newest_per_tag, state_of, version_released,
        write_github_output,
    };
    use crate::assets::MatrixTarget;
    use crate::config::{Assets, Config};
    use crate::git::VersionCommit;
    use crate::github::ReleaseState;
    use crate::publish::PublishEntry;
    use crate::publisher::Upload;
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
            draft: false,
            immutable: false,
            assets: assets.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    fn draft(assets: &[&str]) -> ReleaseState {
        ReleaseState {
            draft: true,
            ..finished(assets)
        }
    }

    fn immutable(assets: &[&str]) -> ReleaseState {
        ReleaseState {
            immutable: true,
            ..finished(assets)
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
                version: "0.2.1".into(),
                listed: false,
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

    /// A draft waits for `release` to publish it, even with every asset.
    #[test]
    fn a_draft_with_every_asset_is_pending_and_builds_nothing() {
        let declared = [target("darwin-arm64"), target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            true,
            &draft(&["demo-darwin-arm64.tar.gz", "demo-linux-x64.tar.gz"]),
            &declared,
            "v0.4.6",
        );
        assert!(!state.complete);
        let pending = group(vec![state]);
        assert_eq!(pending.commits.len(), 1);
        assert_eq!(pending.assets, []);
    }

    #[test]
    fn a_draft_missing_an_asset_builds_only_that_asset() {
        let declared = [target("darwin-arm64"), target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            true,
            &draft(&["demo-linux-x64.tar.gz"]),
            &declared,
            "v0.4.6",
        );
        let pending = group(vec![state]);
        assert_eq!(pending.commits.len(), 1);
        let jobs: Vec<&str> = pending.assets.iter().map(|job| job.id.as_str()).collect();
        assert_eq!(jobs, ["darwin-arm64"]);
    }

    /// A published Release GitHub holds immutable takes no asset, so it
    /// counts complete, plans no asset job, and leaves a warning.
    #[test]
    fn an_immutable_release_missing_an_asset_is_complete_with_a_shortfall() {
        let declared = [target("darwin-arm64"), target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            true,
            &immutable(&["demo-linux-x64.tar.gz"]),
            &declared,
            "v0.4.6",
        );
        assert!(state.complete);
        let pending = group(vec![state]);
        assert!(pending.is_empty());
        assert_eq!(pending.assets, []);
        assert_eq!(
            pending.shortfalls,
            [Shortfall {
                tag: "v0.4.6".into(),
                assets: vec!["demo-darwin-arm64.tar.gz".into()],
            }]
        );
        assert_eq!(
            pending.shortfalls[0].to_string(),
            "release v0.4.6 is published and immutable without demo-darwin-arm64.tar.gz; a later version must ship them"
        );
    }

    /// A published Release GitHub holds mutable still takes assets, so only
    /// the missing target is planned.
    #[test]
    fn a_mutable_release_missing_an_asset_stays_pending_for_that_target() {
        let declared = [target("darwin-arm64"), target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            true,
            &finished(&["demo-linux-x64.tar.gz"]),
            &declared,
            "v0.4.6",
        );
        assert!(!state.complete);
        let pending = group(vec![state]);
        assert_eq!(pending.commits.len(), 1);
        let jobs: Vec<&str> = pending.assets.iter().map(|job| job.id.as_str()).collect();
        assert_eq!(jobs, ["darwin-arm64"]);
        assert_eq!(pending.shortfalls, []);
    }

    /// A commit whose release is done is told apart from one that is not a
    /// version commit, so a `finalize` leg there has nothing to do.
    #[test]
    fn a_finished_commit_is_not_pending() {
        let declared = [target("linux-x64")];
        let done = state_of(
            &entry("core", "0.3.1"),
            at("aaa", 2),
            true,
            &finished(&["demo-linux-x64.tar.gz"]),
            &declared,
            "v0.3.1",
        );
        let waiting = state_of(
            &entry("app", "0.2.1"),
            at("ccc", 0),
            true,
            &draft(&["demo-linux-x64.tar.gz"]),
            &declared,
            "v0.2.1",
        );
        let pending = group(vec![done, waiting]);
        assert!(pending.is_finished("aaa"));
        assert!(!pending.is_finished("ccc"));
        assert!(!pending.is_finished("bbb"));
    }

    /// An immutable Release that holds every asset warns of nothing.
    #[test]
    fn an_immutable_release_with_every_asset_has_no_shortfall() {
        let declared = [target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("aaa", 0),
            true,
            &immutable(&["demo-linux-x64.tar.gz"]),
            &declared,
            "v0.4.6",
        );
        assert_eq!(group(vec![state]).shortfalls, []);
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
        let checked =
            checked_targets(&targets, &held, || Ok(Some(vec!["linux-x64".into()]))).unwrap();
        assert_eq!(ids(&checked), ["linux-x64"]);
        let release = finished(&["demo-linux-x64.tar.gz"]);
        let state = state_of(
            &entry("demo", "0.0.1"),
            at("aaa", 1),
            true,
            &release,
            &checked,
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
            ids(&checked_targets(&targets, &held, declared).unwrap()),
            ["linux-x64", "darwin-arm64"]
        );
    }

    /// An old revision may hold keys the current schema refuses. Only its
    /// target ids are read, so those keys do not matter. A target sits
    /// directly under `assets`, beside the settings, as verctl's own
    /// `.ctl/ver.yaml` writes it; 0.8.1 read an `assets.targets` key no config
    /// holds, and so released with no assets.
    #[test]
    fn an_old_config_names_its_targets_whatever_else_it_holds() {
        let declared: DeclaredTargets = Input::new(
            "old.yaml",
            indoc! {"
                drivers: {}
                a_key_removed_later: true
                assets:
                  bin: demo
                  prepare: [true]
                  build: [true]
                  binary: target/{triple}/release/{bin}
                  linux-x64: {}
                  darwin-arm64:
                    runner: arm64
            "},
        )
        .parse()
        .unwrap();
        assert_eq!(declared.assets.unwrap().0, ["linux-x64", "darwin-arm64"]);
    }

    /// Every setting [`Assets::SETTINGS`] names is a field of the schema, so
    /// none of them can be read as a target.
    #[test]
    fn the_asset_settings_match_the_schema() {
        let packed = indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
            assets:
              bin: demo
              prepare: [rustup]
              build: [cargo]
              dir: dist
              files: [demo]
              linux-x64: {}
        "};
        let single = indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
            assets:
              binary: target/{triple}/release/{bin}
              linux-x64: {}
        "};
        let mut seen = Vec::new();
        for yaml in [packed, single] {
            let config = Config::parse(yaml).unwrap();
            let assets = config.assets.unwrap();
            assert_eq!(assets.targets.keys().collect::<Vec<_>>(), ["linux-x64"]);
            let ids = Input::new("config.yaml", yaml)
                .parse::<DeclaredTargets>()
                .unwrap()
                .assets
                .unwrap()
                .0;
            assert_eq!(ids, ["linux-x64"], "{yaml}");
            seen.extend(
                Assets::SETTINGS
                    .iter()
                    .copied()
                    .filter(|key| yaml.contains(&format!("  {key}:"))),
            );
        }
        seen.sort_unstable();
        let mut settings = Assets::SETTINGS.to_vec();
        settings.sort_unstable();
        assert_eq!(seen, settings, "each setting is exercised once");
    }

    #[test]
    fn a_commit_with_no_config_owes_no_asset() {
        let targets = [target("linux-x64")];
        assert_eq!(
            ids(&checked_targets(&targets, &[], || Ok(None)).unwrap()),
            Vec::<&str>::new()
        );
    }

    /// The old config is read only for a release that lacks an asset, so a
    /// complete release never needs it to parse.
    #[test]
    fn a_complete_release_never_reads_its_old_config() {
        let targets = [target("linux-x64")];
        let held = ["demo-linux-x64.tar.gz".to_owned()];
        let checked = checked_targets(&targets, &held, || {
            panic!("the old config was read for a complete release")
        })
        .unwrap();
        assert_eq!(ids(&checked), ["linux-x64"]);
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
            fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                commits=["aaa"]
                pending=true
                assets={"include":[]}
                declared={"aaa":""}
            "#}
        );
        let nothing = dir.path().join("nothing");
        write_github_output(&Pending::default(), &nothing).unwrap();
        assert_eq!(
            fs::read_to_string(&nothing).unwrap(),
            indoc! {r#"
                commits=[]
                pending=false
                assets={"include":[]}
                declared={}
            "#}
        );
    }

    /// Each pending commit lists the assets its Release is expected to hold,
    /// so a leg at that commit declares what the plan scheduled, even when the
    /// commit's own config names more.
    #[test]
    fn the_plan_hands_each_commit_its_expected_assets() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("output");
        let declared = [target("darwin-arm64"), target("linux-x64")];
        let state = state_of(
            &entry("demo", "0.4.6"),
            at("bbb", 0),
            true,
            &draft(&["demo-linux-x64.tar.gz"]),
            &declared,
            "v0.4.6",
        );
        let pending = group(vec![incomplete("core", "0.3.1", at("aaa", 1)), state]);
        write_github_output(&pending, &path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains(
                r#"declared={"aaa":"","bbb":"demo-darwin-arm64.tar.gz,demo-linux-x64.tar.gz"}"#
            ),
            "{written}"
        );
    }

    fn query(argv: &[&str]) -> RegistryCheck {
        RegistryCheck::Query {
            argv: argv.iter().map(|word| (*word).to_owned()).collect(),
            dir: env::temp_dir(),
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

    fn member(name: &str, version: &str) -> Upload {
        Upload {
            name: name.to_owned(),
            version: version.to_owned(),
            manifest: PathBuf::from(format!("{name}/Cargo.toml")),
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
