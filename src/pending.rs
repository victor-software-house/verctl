//! Which releases HEAD still has to make.
//!
//! A release is complete when the registry lists each package's version, and
//! its tag and its published Release exist, with every declared asset.
//! `release` creates a draft with no tag first and publishes it after every
//! upload, which creates the tag.
//! A draft is incomplete, and `release` run again publishes it.
//! A published Release that lacks an asset is incomplete too, unless GitHub
//! holds it immutable.
//! An immutable Release takes no asset, so it counts complete with a
//! [`Shortfall`] to warn about.
//!
//! HEAD is the release reference.
//! The plan holds the versions HEAD's manifests hold, whichever commit set
//! them, and no other version.
//! `status` lists them, and `release` releases them from HEAD.

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
use crate::github::{self, ReleaseList, ReleaseState};
use crate::publish::{self, PublishEntry};
use crate::publisher::Upload;
use crate::registry::RegistryCheck;
use crate::registry_token::RegistryToken;
use crate::{bump, publisher, stop};

/// The incomplete releases of the versions HEAD holds, and the assets missing
/// from them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Pending {
    /// The commits that hold incomplete releases: HEAD, or none.
    pub commits: Vec<PendingCommit>,
    /// Asset jobs to run: one per missing target.
    pub assets: Vec<PendingAsset>,
    /// Published Releases that are immutable and lack declared assets. They
    /// count complete, and no asset job is planned for them.
    pub shortfalls: Vec<Shortfall>,
    /// Releases whose registries list some, but not all, of their uploads.
    pub partials: Vec<Partial>,
}

/// A release whose registries list some, but not all, of the uploads that
/// have a registry check, such as a run that crates.io stopped with a 429
/// partway through.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Partial {
    /// The release tag.
    pub tag: String,
    /// The uploads with a registry check that their registry lists. `None`
    /// when a lookup after the first unlisted upload failed, so the count is
    /// not known; the plan still holds the release as incomplete.
    pub live: Option<usize>,
    /// Every upload with a registry check under the tag's packages. A
    /// workspace package counts each member.
    pub total: usize,
    /// What they are, plural: the publisher's noun when every package under
    /// the tag shares it, such as `crates`, and `packages` otherwise.
    pub noun: String,
    /// The stop record of the tag's draft, as the last run that failed an
    /// upload wrote it. Display only.
    pub stop: stop::Record,
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

    /// The pending commit at `sha`, if any.
    #[must_use]
    pub fn at(&self, sha: &str) -> Option<&PendingCommit> {
        self.commits
            .iter()
            .find(|commit| commit.sha.eq_ignore_ascii_case(sha))
    }
}

/// A commit and the packages it still has to release.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PendingCommit {
    /// Full hex SHA of the commit.
    pub sha: String,
    /// The incomplete packages released at this commit.
    pub packages: Vec<PendingPackage>,
    /// The file names of the assets this commit's Release is expected to
    /// hold, drafts and missing ones alike. The workflow hands them to each
    /// leg, which declares exactly these.
    pub declared: Vec<String>,
}

/// A package whose release is incomplete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PendingPackage {
    /// Package name.
    pub name: String,
    /// The version HEAD's manifest holds.
    pub version: String,
    /// Whether the registry already lists the version or, for an upload with
    /// no registry check, its tag or its published Release exists. Only the
    /// Release or its assets are then left, such as a draft waiting to be
    /// published, and `release` runs no upload for it.
    #[serde(skip)]
    pub listed: bool,
}

/// One asset job: a target whose tarball the Release lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PendingAsset {
    /// The commit the job checks out.
    pub commit: String,
    /// The target id, such as `darwin-arm64`.
    pub id: String,
    /// The machine labels the job runs on.
    pub labels: Vec<String>,
    /// The release tag the tarball is uploaded to.
    pub tag: String,
    /// The tarball's file name, which `release asset` writes beside the
    /// config's root, so a job can hand the file on without parsing output.
    pub asset: String,
}

/// One package's state, before grouping.
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

/// Find every incomplete release of a version HEAD's manifests hold.
///
/// The plan commit is HEAD for every package. A release tag only has to sit at
/// HEAD or at a first-parent commit at the same version, so a finished
/// release whose tag names an older commit stays complete.
///
/// A package at `0.0.0` is left out before any lookup: it has never been
/// released, and its first release is the `0.0.1` a Version PR prepares (see
/// [`bump::unreleased`]).
///
/// Every package is planned at HEAD, whichever commit set its version.
/// A version HEAD's manifests do not hold is never looked up.
///
/// With `skip_assets`, declared assets do not count toward completeness, so a
/// repository that turns assets off still reaches nothing to release.
/// A non-empty `declared_assets` keeps only the targets whose tarball it names,
/// as `release --declared-assets` does.
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
/// - HEAD cannot be read (see [`git::head_sha`]);
/// - the repository has no GitHub `origin`;
/// - a registry or GitHub lookup fails.
///
/// A lookup that cannot run is a [`crate::lookup::Unavailable`]. It covers a
/// missing GitHub repository, no answer from GitHub or a registry, and a walk
/// that reaches a shallow clone's boundary.
///
/// It also returns an error when a package's tag names the wrong commit. The
/// right commit is HEAD, or another commit HEAD contains at the same version
/// (see [`git::tagged_commit`]). Counting a release tagged in the wrong place
/// as complete would hide it.
///
/// `list` holds the Releases list for every tag the plan looks up, so a
/// run that goes on to release reads it once.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one input of the plan; a struct would be built once per call"
)]
pub fn pending(
    config: &Config,
    config_path: &Path,
    root: &Path,
    token: &str,
    skip_assets: bool,
    declared_assets: &[String],
    registry_token: Option<&RegistryToken>,
    list: &ReleaseList,
) -> Result<Pending> {
    if config.packages.is_empty() {
        return Ok(Pending::default());
    }
    let planned = publish::plan(config, root)?;
    let repo = github::repo(root)?;
    let head = git::head_sha(root)?;
    let (targets, assets_tag) = if skip_assets || config.assets.is_none() {
        (Vec::new(), String::new())
    } else {
        let plan = assets::plan(config, root)?;
        let include = plan
            .matrix
            .include
            .into_iter()
            .filter(|target| declared_assets.is_empty() || declared_assets.contains(&target.asset))
            .collect();
        (include, plan.tag)
    };
    let env = registry_token.map(RegistryToken::env);
    let env = env.as_slice();
    let mut states = Vec::new();
    let mut releases: BTreeMap<String, ReleaseState> = BTreeMap::new();
    let mut counts: BTreeMap<String, Count> = BTreeMap::new();
    for (index, (spec, entry)) in config.packages.iter().zip(&planned.packages).enumerate() {
        if bump::unreleased(&entry.version) {
            continue;
        }
        let driver = spec.resolve(config, root)?;
        let tag = tag_of(&planned, entry)?;
        let release = match releases.entry(tag.clone()) {
            Entry::Occupied(found) => found.into_mut(),
            Entry::Vacant(slot) => {
                let state = github::release_state(token, &repo, slot.key(), list)?;
                slot.insert(state)
            }
        };
        let release_commit = match &release.tag {
            Some(named) if !named.eq_ignore_ascii_case(&head) => {
                git::tagged_commit(root, &root.join(&spec.path), named, &|raw: &str| {
                    version_in(&driver, raw)
                })?
                .with_context(|| {
                    format!(
                        "tag {tag} names {named}: not HEAD {head} and not a first-parent commit \
                         at {}@{}",
                        entry.name, entry.version
                    )
                })?
                .sha
            }
            Some(_) | None => head.clone(),
        };
        let commit = VersionCommit {
            sha: head.clone(),
            depth: 0,
        };
        let count = release
            .draft
            .then(|| counts.entry(tag.clone()).or_default());
        let listed = registry_lists(config, root, spec, entry, release, env, count)?;
        let head_targets: &[MatrixTarget] = if index == 0 { &targets } else { &[] };
        let declared = checked_targets(head_targets, &release.assets, || {
            declared_at(root, config_path, &release_commit)
        })?;
        states.push(state_of(
            entry,
            commit,
            listed,
            release,
            &declared,
            &assets_tag,
        ));
    }
    let partials = counts
        .into_iter()
        .filter_map(|(tag, count)| {
            let stop = releases.get(&tag).map(|release| release.stop.clone())?;
            count.partial(tag, stop)
        })
        .collect();
    Ok(Pending {
        partials,
        ..group(states)
    })
}

/// The uploads with a registry check under one tag whose Release is a
/// draft, so a run has started, and how many are live.
#[derive(Default)]
struct Count {
    live: usize,
    total: usize,
    nouns: Vec<String>,
    /// An upload under the tag is unlisted.
    unlisted: bool,
    /// A lookup after the first unlisted upload failed, so `live` is not
    /// known.
    uncounted: bool,
}

impl Count {
    /// Ask `check` whether it lists `version`, for an upload whose publisher
    /// calls it `noun`, and count the answer.
    ///
    /// Until an upload is unlisted, a failed lookup fails the plan, as the
    /// lookup of a release that has not started does. After it, the release
    /// is known to be incomplete, so a failed lookup only leaves the count
    /// unknown and that upload unlisted. The tag's later uploads are still
    /// asked, so a package that shares the tag and is live reads as listed.
    fn ask(
        &mut self,
        check: &RegistryCheck,
        version: &str,
        env: &[(&str, &str)],
        noun: &str,
    ) -> Result<bool> {
        self.total += 1;
        if !self.nouns.iter().any(|known| known == noun) {
            self.nouns.push(noun.to_owned());
        }
        let live = match check.lists(version, env) {
            Ok(live) => live,
            Err(_) if self.unlisted => {
                self.uncounted = true;
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        if live {
            self.live += 1;
        } else {
            self.unlisted = true;
        }
        Ok(live)
    }

    /// The partial release of `tag`: at least one upload live, and either
    /// some not live or a count left unknown. With no upload known to be
    /// live, nothing shows the release is partial.
    fn partial(self, tag: String, stop: stop::Record) -> Option<Partial> {
        let noun = match self.nouns.as_slice() {
            [one] => format!("{one}s"),
            _ => "packages".to_owned(),
        };
        let live = (!self.uncounted).then_some(self.live);
        (self.live > 0 && (self.uncounted || self.live < self.total)).then_some(Partial {
            tag,
            live,
            total: self.total,
            noun,
            stop,
        })
    }
}

/// The asset targets a release is checked for. HEAD declares `targets`, and
/// the release is checked only for those its release commit declared too. A
/// target added after the release never reopens it.
///
/// A release that already holds every target's asset is checked for them all.
/// `declared` is then never called, so its old config never has to parse.
/// Otherwise `declared` gives the target ids the release commit's config
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
/// With no `count`, the answer stops at the first unlisted upload. A `count`
/// is given when the tag's Release is a draft: a run has started, so every
/// upload is asked and counted, for `status` to report the release as
/// partial (see [`Count::ask`]).
///
/// A package with no registry counts as listed. An upload with no check
/// counts as listed once its tag or its published Release exists: `release`
/// publishes the draft, which creates the tag, only after every upload, so
/// either is the record. GitHub can serve the tag a moment after the Release,
/// so a published Release whose tag is not served yet counts too.
///
/// A `query` runs with `env`, as [`RegistryCheck::lists`] describes.
fn registry_lists(
    config: &Config,
    root: &Path,
    spec: &PackageSpec,
    entry: &PublishEntry,
    release: &ReleaseState,
    env: &[(&str, &str)],
    count: Option<&mut Count>,
) -> Result<bool> {
    if entry.argv.is_empty() {
        return Ok(true);
    }
    let uploads = publisher::uploads(config, spec, root, entry)?;
    uploads_listed(&uploads, &entry.noun, release, env, count)
}

/// Whether every one of `uploads` is listed, as [`registry_lists`] answers
/// it.
fn uploads_listed(
    uploads: &[Upload],
    noun: &str,
    release: &ReleaseState,
    env: &[(&str, &str)],
    mut count: Option<&mut Count>,
) -> Result<bool> {
    let mut listed = true;
    for upload in uploads {
        let live = match (&upload.check, count.as_deref_mut()) {
            (RegistryCheck::Unchecked, _) => {
                release.tag.is_some() || (release.release && !release.draft)
            }
            (check @ (RegistryCheck::CratesIo { .. } | RegistryCheck::Query { .. }), None) => {
                check.lists(&upload.version, env)?
            }
            (
                check @ (RegistryCheck::CratesIo { .. } | RegistryCheck::Query { .. }),
                Some(count),
            ) => count.ask(check, &upload.version, env, noun)?,
        };
        listed &= live;
        if !listed && count.is_none() {
            return Ok(false);
        }
    }
    Ok(listed)
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

/// The incomplete packages at one commit, while grouping.
struct AtCommit {
    packages: Vec<PendingPackage>,
    missing: Vec<(MatrixTarget, String)>,
    declared: Vec<String>,
}

/// Group incomplete packages by the commit that releases them. Assets belong
/// to the first package, so each asset job uploads to the tag of that
/// package's version.
fn group(states: Vec<PackageState>) -> Pending {
    let mut by_commit: BTreeMap<String, AtCommit> = BTreeMap::new();
    let mut shortfalls = Vec::new();
    for mut state in states {
        shortfalls.extend(state.shortfall.take());
        if state.complete {
            continue;
        }
        let slot = by_commit
            .entry(state.commit.sha)
            .or_insert_with(|| AtCommit {
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
    let mut pending = Pending {
        shortfalls,
        ..Pending::default()
    };
    for (sha, at) in by_commit {
        pending
            .assets
            .extend(at.missing.into_iter().map(|(target, tag)| PendingAsset {
                commit: sha.clone(),
                id: target.id,
                labels: target.labels,
                tag,
                asset: target.asset,
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

/// Append `commits` (a JSON list of HEAD, or empty), `pending`, `assets` (a
/// job matrix of commit, target id, and labels), and `declared` to the step
/// output file `path`.
/// `declared` is the comma-separated file names of the assets HEAD's Release
/// is expected to hold, empty when there are none or nothing is pending, so a
/// leg declares the same set the plan scheduled.
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
    let declared = pending
        .commits
        .iter()
        .flat_map(|commit| commit.declared.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(",");
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
    use std::fs;
    use std::path::Path;

    use ctl_core::input::Input;
    use indoc::{formatdoc, indoc};

    use super::{
        Count, DeclaredTargets, PackageState, Pending, PendingAsset, PendingPackage, Shortfall,
        checked_targets, group, state_of, uploads_listed, write_github_output,
    };
    use crate::assets::MatrixTarget;
    use crate::config::{Assets, Config};
    use crate::git::VersionCommit;
    use crate::github::ReleaseState;
    use crate::publish::PublishEntry;
    use crate::publisher::Upload;
    use crate::registry::RegistryCheck;
    use crate::stop::Record;

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
            stop: Record::Absent,
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

    /// How a test upload's registry query answers.
    #[derive(Clone, Copy)]
    enum Answer {
        Listed,
        Unlisted,
        /// The query cannot run, so the lookup fails.
        Fails,
    }

    /// An upload named `name` whose query appends `name` to `dir/calls`, then
    /// answers as `answer` says.
    fn upload(dir: &Path, name: &str, answer: Answer) -> Upload {
        let script = dir.join(format!("{name}.sh"));
        let reply = match answer {
            Answer::Listed => "echo 0.4.0",
            Answer::Unlisted | Answer::Fails => "exit 1",
        };
        fs::write(
            &script,
            formatdoc! {r#"
                echo {name} >> "{calls}"
                {reply}
            "#, calls = dir.join("calls").display()},
        )
        .unwrap();
        let argv = match answer {
            Answer::Listed | Answer::Unlisted => {
                vec!["sh".to_owned(), script.display().to_string()]
            }
            Answer::Fails => vec![dir.join("no-such-query").display().to_string()],
        };
        Upload {
            name: name.into(),
            version: "0.4.0".into(),
            manifest: dir.join("Cargo.toml"),
            argv: vec!["true".into()],
            check: RegistryCheck::Query {
                argv,
                dir: dir.to_path_buf(),
            },
        }
    }

    fn calls(dir: &Path) -> String {
        fs::read_to_string(dir.join("calls")).unwrap_or_default()
    }

    /// A release whose Release is no draft has not started, so the answer
    /// stops at the first unlisted upload and the rest are never asked.
    #[test]
    fn a_release_not_started_asks_up_to_its_first_unlisted_upload() {
        let dir = tempfile::TempDir::new().unwrap();
        let uploads = [
            upload(dir.path(), "a", Answer::Listed),
            upload(dir.path(), "b", Answer::Unlisted),
            upload(dir.path(), "c", Answer::Listed),
        ];
        let listed =
            uploads_listed(&uploads, "crate", &ReleaseState::default(), &[], None).unwrap();
        assert!(!listed);
        assert_eq!(
            calls(dir.path()),
            indoc! {"
                a
                b
            "}
        );
    }

    /// A draft means a run started, so every upload is counted; one crate of
    /// three live is partial, and their shared noun names them.
    #[test]
    fn some_crates_live_is_a_partial_release() {
        let dir = tempfile::TempDir::new().unwrap();
        let uploads = [
            upload(dir.path(), "a", Answer::Listed),
            upload(dir.path(), "b", Answer::Unlisted),
            upload(dir.path(), "c", Answer::Unlisted),
        ];
        let mut count = Count::default();
        let listed = uploads_listed(&uploads, "crate", &draft(&[]), &[], Some(&mut count)).unwrap();
        assert!(!listed);
        let partial = count.partial("v0.4.0".into(), Record::Unreadable).unwrap();
        assert_eq!((partial.live, partial.total), (Some(1), 3));
        assert_eq!(partial.noun, "crates");
        assert_eq!(partial.stop, Record::Unreadable);
    }

    /// A lookup that fails after the first unlisted upload leaves the count
    /// unknown, and the plan goes on: the release is incomplete either way.
    #[test]
    fn a_started_release_still_plans_when_a_later_lookup_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let uploads = [
            upload(dir.path(), "a", Answer::Listed),
            upload(dir.path(), "b", Answer::Unlisted),
            upload(dir.path(), "c", Answer::Fails),
        ];
        let mut count = Count::default();
        let listed = uploads_listed(&uploads, "crate", &draft(&[]), &[], Some(&mut count)).unwrap();
        assert!(!listed);
        let partial = count.partial("v0.4.0".into(), Record::Absent).unwrap();
        assert_eq!((partial.live, partial.total), (None, 3));
    }

    /// A failed lookup does not stop the tag's later lookups, so a package
    /// sharing the tag that is live still reads as listed.
    #[test]
    fn a_failed_lookup_leaves_later_packages_asked() {
        let dir = tempfile::TempDir::new().unwrap();
        let release = draft(&[]);
        let mut count = Count::default();
        let first = [
            upload(dir.path(), "a", Answer::Unlisted),
            upload(dir.path(), "b", Answer::Fails),
        ];
        assert!(!uploads_listed(&first, "crate", &release, &[], Some(&mut count)).unwrap());
        let second = [upload(dir.path(), "c", Answer::Listed)];
        assert!(uploads_listed(&second, "crate", &release, &[], Some(&mut count)).unwrap());
        assert_eq!(
            calls(dir.path()),
            indoc! {"
                a
                c
            "}
        );
        let partial = count.partial("v0.4.0".into(), Record::Absent).unwrap();
        assert_eq!((partial.live, partial.total), (None, 3));
    }

    /// An unknown count with no upload known to be live is not partial.
    #[test]
    fn an_unknown_count_with_nothing_live_is_not_partial() {
        let dir = tempfile::TempDir::new().unwrap();
        let uploads = [
            upload(dir.path(), "a", Answer::Unlisted),
            upload(dir.path(), "b", Answer::Fails),
        ];
        let mut count = Count::default();
        uploads_listed(&uploads, "crate", &draft(&[]), &[], Some(&mut count)).unwrap();
        assert!(count.partial("v0.4.0".into(), Record::Absent).is_none());
    }

    /// Before any upload is unlisted, a failed lookup still fails the plan.
    #[test]
    fn a_failed_first_lookup_fails_the_plan() {
        let dir = tempfile::TempDir::new().unwrap();
        let uploads = [upload(dir.path(), "a", Answer::Fails)];
        let mut count = Count::default();
        assert!(uploads_listed(&uploads, "crate", &draft(&[]), &[], Some(&mut count)).is_err());
    }

    /// Mixed nouns under one tag read as packages.
    #[test]
    fn mixed_nouns_are_packages() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut count = Count::default();
        let release = draft(&[]);
        uploads_listed(
            &[upload(dir.path(), "a", Answer::Listed)],
            "crate",
            &release,
            &[],
            Some(&mut count),
        )
        .unwrap();
        uploads_listed(
            &[upload(dir.path(), "b", Answer::Unlisted)],
            "package",
            &release,
            &[],
            Some(&mut count),
        )
        .unwrap();
        let partial = count.partial("v1.0.0".into(), Record::Absent).unwrap();
        assert_eq!(partial.noun, "packages");
    }

    #[test]
    fn none_or_every_upload_live_is_not_partial() {
        for answer in [Answer::Listed, Answer::Unlisted] {
            let dir = tempfile::TempDir::new().unwrap();
            let uploads = [
                upload(dir.path(), "a", answer),
                upload(dir.path(), "b", answer),
            ];
            let mut count = Count::default();
            uploads_listed(&uploads, "crate", &draft(&[]), &[], Some(&mut count)).unwrap();
            assert!(count.partial("v1.0.0".into(), Record::Absent).is_none());
        }
    }

    /// Two packages whose versions different commits set are one entry of the
    /// plan, at HEAD.
    #[test]
    fn packages_bumped_at_different_commits_are_planned_at_head() {
        let pending = group(vec![
            incomplete("core", "0.3.1", at("head", 0)),
            incomplete("app", "0.2.1", at("head", 0)),
        ]);
        let shas: Vec<&str> = pending.commits.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(shas, ["head"]);
        let packages = &pending.at("head").unwrap().packages;
        assert_eq!(
            packages,
            &[
                PendingPackage {
                    name: "core".into(),
                    version: "0.3.1".into(),
                    listed: false,
                },
                PendingPackage {
                    name: "app".into(),
                    version: "0.2.1".into(),
                    listed: false,
                },
            ]
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
                asset: "demo-darwin-arm64.tar.gz".into(),
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
        let pending = group(vec![incomplete("demo", "0.4.6", at("aaa", 0))]);
        write_github_output(&pending, &path).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                commits=["aaa"]
                pending=true
                assets={"include":[]}
                declared=
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
                declared=
            "#}
        );
    }

    /// The plan hands the legs the assets HEAD's Release is expected to hold,
    /// as comma-separated file names, even when HEAD's own config names more.
    #[test]
    fn the_plan_hands_the_legs_the_expected_assets() {
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
        let pending = group(vec![incomplete("core", "0.3.1", at("bbb", 0)), state]);
        write_github_output(&pending, &path).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("declared=demo-darwin-arm64.tar.gz,demo-linux-x64.tar.gz"),
            "{written}"
        );
    }
}
