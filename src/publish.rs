//! Publish the versions already on HEAD and open a GitHub Release.
//!
//! How each package is published comes from `[publishers.*]`.
//! Cargo and bun are stock recipes, and other stacks work as well.
//! How tags are named comes from `[tags].template`; see [`tags_for`].

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fmt, slice};

use anyhow::{Context, Result, bail, ensure};
use ctl_core::formatdoc;
use serde::Serialize;

use crate::config::{Config, NAME_PLACEHOLDER, PackageSpec};
use crate::publisher::Upload;
use crate::registry::{self, CratesIo, RegistryCheck};
use crate::registry_check::{Proof, ProofRow};
use crate::registry_token::RegistryToken;
use crate::{
    assets, bump, git, github, pending, process, publisher, registry_check, release, stop,
};

/// How long one package upload may run.
pub const PUBLISH_TIMEOUT: Duration = Duration::from_mins(10);

/// What `publish` will do: the packages to upload and the tags to release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPlan {
    /// Packages in config order.
    pub packages: Vec<PublishEntry>,
    /// Tags in the order they are created.
    pub tags: Vec<TagPlan>,
}

/// One tag the release will create, and the packages whose notes fill it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPlan {
    /// Tag name.
    pub tag: String,
    /// Release title.
    pub title: String,
    /// Packages whose notes fill this tag's Release.
    pub packages: Vec<PublishEntry>,
}

/// A package and its version on HEAD, plus the command that uploads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishEntry {
    /// Package name.
    pub name: String,
    /// Version read from the manifest.
    pub version: String,
    /// What the registry calls the package, such as `crate`.
    pub noun: String,
    /// Publisher and registry, as shown to the operator.
    pub label: String,
    /// The upload command. Empty for a `publisher: none` package, which only
    /// gets its tag and Release. Such a package never counts as shipped: with
    /// no registry to ask, an existing Release is the proof it was released.
    pub argv: Vec<String>,
}

/// What a publish run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOutcome {
    /// One line per package.
    pub packages: Vec<PublishLine>,
    /// The Release URL for each tag, or `would create <tag>` in a dry run.
    pub releases: Vec<String>,
}

/// A package's row in the publish report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublishLine {
    /// Package name.
    pub name: String,
    /// Version published.
    pub version: String,
    /// Publisher and registry used.
    pub via: String,
    /// `already` when the registry had this version. `None` otherwise.
    pub note: Option<String>,
}

/// Resolve each package's version, upload command, and the tags to release.
///
/// A package at `0.0.0` gets no tag, so a new package beside a released one
/// does not break a shared `v{version}` tag (see [`bump::unreleased`]).
///
/// # Errors
///
/// Returns an error when a package's version driver cannot be resolved, its
/// manifest cannot be read, or the driver finds no version in it. It also
/// fails when no publisher resolves for a package (see
/// [`publisher::resolve`]) or when [`tags_for`] rejects the packages.
pub fn plan(config: &Config, root: &Path) -> Result<PublishPlan> {
    let mut packages = Vec::new();
    for spec in &config.packages {
        let current = spec.current(config, root)?;
        let version = current.version;
        let (label, noun, argv) = publisher::resolve_for_version(config, spec, root, &version)?;
        packages.push(PublishEntry {
            name: spec.name.clone(),
            version,
            noun,
            label,
            argv,
        });
    }
    let released: Vec<PublishEntry> = packages
        .iter()
        .filter(|entry| !bump::unreleased(&entry.version))
        .cloned()
        .collect();
    let tags = if released.is_empty() && !packages.is_empty() {
        Vec::new()
    } else {
        tags_for(config, &released)?
    };
    Ok(PublishPlan { packages, tags })
}

/// Open a draft Release per tag, publish each package, then publish the
/// drafts. A dry run only plans.
///
/// This is [`run_pending`] with assets counted toward completeness, no
/// assets directory, and no registry token, so each publisher finds its own
/// credential. It reads crates.io itself, with no owner's token, so the
/// release proof reports `trust` as skipped.
///
/// # Errors
///
/// Returns an error when [`run_pending`] fails.
pub fn run(
    config: &Config,
    config_path: &Path,
    root: &Path,
    dry_run: bool,
) -> Result<PublishOutcome> {
    Ok(run_pending(
        config,
        config_path,
        root,
        dry_run,
        &Scope::default(),
        Allow::default(),
        None,
        CratesIo::PUBLIC,
    )?
    .published)
}

/// The proofs a run may go past, each refused unless the operator allows it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Allow {
    /// Publish when `.github/workflows/` at HEAD differs from the default
    /// branch's tip.
    pub workflow_change: bool,
    /// Publish from a commit the default branch does not contain.
    pub off_default_branch: bool,
}

/// What a `release` run leaves out, declares, or starts, beyond the packages
/// HEAD holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    /// Leave declared assets out of the completeness check and out of the
    /// draft, so the draft is published without them.
    pub skip_assets: bool,
    /// The asset file names the release workflow's plan expects at HEAD. The
    /// run declares exactly these.
    /// Empty declares the targets HEAD's own config names.
    pub declared_assets: Vec<String>,
    /// Create and publish tags and Releases only. The run starts no publisher
    /// and refuses a package the registry does not list.
    pub releases_only: bool,
    /// The directory holding the tarballs `release asset` built. The run
    /// uploads from it each declared tarball the draft, or a mutable Release
    /// published at HEAD, lacks.
    /// `None` uploads nothing, and the run then fails when that Release lacks
    /// a declared tarball.
    pub assets_dir: Option<PathBuf>,
    /// Create each crate crates.io lacks with the registry token, where trusted
    /// publishing publishes the rest. The release proof then passes such a
    /// crate with a notice, and a standalone package at `0.0.0` that crates.io
    /// lacks joins the plan.
    pub bootstrap: bool,
}

/// A proof that would have refused the run, which the operator allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Allowed {
    /// The proof the run went past.
    pub check: Check,
    /// What the proof found.
    pub reason: String,
}

/// A proof the operator can allow.
///
/// The binary prints each one through [`Check::name`] and matches none, so a
/// new variant adds no arm there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Check {
    /// `.github/workflows/` differs from the default branch's tip.
    WorkflowChange,
    /// The default branch does not contain HEAD.
    OffDefaultBranch,
}

impl Check {
    /// The name the report and the JSON use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WorkflowChange => "workflow-change",
            Self::OffDefaultBranch => "off-default-branch",
        }
    }

    /// The flag that allows it.
    const fn flag(self) -> &'static str {
        match self {
            Self::WorkflowChange => "--allow-workflow-change",
            Self::OffDefaultBranch => "--allow-off-default-branch",
        }
    }
}

/// Whether a publish run found anything to release.
///
/// Not `#[non_exhaustive]`: the binary prints each outcome, and an exhaustive
/// match makes a new variant fail the build until it is printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Every package's registry entry, tag, Release, and counted assets
    /// already exist, so the run did nothing.
    NothingToRelease,
    /// The run released the incomplete packages HEAD holds.
    Released,
}

/// What a publish run did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PublishRun {
    /// Whether there was anything to release.
    pub outcome: Outcome,
    /// The packages and Releases this run handled.
    pub published: PublishOutcome,
    /// The proofs the operator allowed this run to go past.
    pub allowed: Vec<Allowed>,
    /// Published, immutable Releases that lack declared assets and can take
    /// none.
    pub shortfalls: Vec<pending::Shortfall>,
    /// What the release proof found beyond a pass: its notices, the checks it
    /// skipped, and, in a dry run only, its failures. Empty when the run
    /// found nothing to release, which runs no proof.
    pub proof: Vec<ProofRow>,
}

/// Release, from HEAD, every package version HEAD holds whose release is
/// incomplete, whichever commit set it, and report nothing to release when
/// every release is complete.
/// A version HEAD's manifests do not hold is never released.
///
/// Every reversible step comes first, so a failed upload leaves no tag:
///
/// 1. Each tag gets a draft Release at HEAD, with no tag. A draft found at
///    another commit is deleted and created again; a draft at HEAD is used. A
///    tag that exists must name HEAD.
/// 2. Each declared tarball the draft, or a mutable Release published at HEAD,
///    lacks is uploaded to it from `scope.assets_dir`.
/// 3. Each package is published to its registry, in config order, and a version
///    the registry lists is skipped.
/// 4. Each draft is published, which creates its tag at HEAD.
/// 5. Each tag is read until it names HEAD.
///
/// GitHub is read for every tag, and every declared tarball is found, before
/// the run writes anything.
/// A Release already published at HEAD gets neither step 1 nor step 4.
/// A mutable one takes the declared tarballs it lacks in step 2, and an
/// immutable one takes none.
///
/// With `scope.skip_assets`, declared assets do not count toward completeness.
/// `scope.declared_assets` names the assets the run declares, and
/// `scope.releases_only` keeps every publisher from starting.
/// A package at `0.0.0` needs no changelog section, and is released only as
/// [`pending::pending`] describes: a standalone crate that crates.io lists
/// without a tag, or, with `scope.bootstrap`, one that crates.io lacks. Its
/// tag renders over that package alone ([`zero_tags`]).
/// A dry run plans every other package and runs the release proof. It reads
/// no registry listing.
///
/// Before any publisher, tag, or Release call, the run proves HEAD in two
/// ways:
///
/// - The default branch contains it.
/// - Its `.github/workflows/` matches the tip of the default branch.
///
/// `allow` lets the run go past either.
/// [`PublishRun::allowed`] lists each one the run goes past.
///
/// Once the plan finds something left to release, and before the first
/// write, the run proves what can change on the registry after the merge:
/// each check `release_checks` names, against the crates.io API in
/// `crates_io` ([`registry_check::prove`]). `trust` runs only with its owner's
/// token.
///
/// A failed check stops the run before the draft, with nothing published. A
/// complete release runs no proof. The completeness check and the check
/// after each upload read the crates.io index in `crates_io`.
///
/// # Errors
///
/// Returns an error when:
///
/// - The plan cannot be resolved.
/// - A changelog lacks the heading for a package version.
/// - A proof refuses or cannot run, or a check of the release proof fails.
/// - No token is set.
/// - A registry or GitHub lookup fails.
/// - The release plan names a commit other than HEAD.
/// - A tag, or a published Release, is at a commit other than HEAD.
/// - A declared tarball is neither on the draft nor in `scope.assets_dir`.
/// - A draft, asset, publisher, or tag step fails.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one input of the release; a struct would be built once per call"
)]
pub fn run_pending(
    config: &Config,
    config_path: &Path,
    root: &Path,
    dry_run: bool,
    scope: &Scope,
    allow: Allow,
    registry_token: Option<&RegistryToken>,
    crates_io: CratesIo<'_>,
) -> Result<PublishRun> {
    let planned = plan(config, root)?;
    let declared = declared_assets(config, root, scope)?;
    if dry_run {
        return dry_run_plan(
            config,
            root,
            &planned,
            &declared,
            crates_io,
            scope.bootstrap,
        );
    }
    full_history(root)?;
    let allowed = prove_version_commit(config, root, &planned, allow)?;
    let token = release::resolve_token()?;
    let release_list = github::ReleaseList::default();
    let pending = pending::pending(
        config,
        config_path,
        root,
        &token,
        scope.skip_assets,
        &scope.declared_assets,
        registry_token,
        crates_io.index,
        scope.bootstrap,
        &release_list,
    )?;
    if pending.is_empty() {
        return Ok(nothing_to_release(allowed, pending.shortfalls));
    }
    let proof = release_proof(config, root, crates_io, scope.bootstrap)?;
    let failures = proof.failures();
    ensure!(
        failures.is_empty(),
        "the release proof failed, and nothing was published:\n{}",
        failures.join("\n")
    );
    let head = git::head_sha(root)?;
    let here = at_head(&pending, &head)?;
    let selected = selected_packages(config, &planned, here);
    let (uploads, already) =
        gather_uploads(config, root, here, &selected, scope.releases_only, &head)?;
    let repo = github::repo(root)?;
    let zero_plans = zero_tags(config, &selected_zero(&selected))?;
    let tags = planned
        .tags
        .iter()
        .chain(&zero_plans)
        .filter(|tag| {
            tag.packages.iter().any(|named| {
                here.packages
                    .iter()
                    .any(|package| package.name == named.name)
            })
        })
        .map(|tag| TagRelease::read(config, root, &token, &repo, tag, &head, &release_list))
        .collect::<Result<Vec<_>>>()?;
    let files = asset_files(&tags, &declared, scope.assets_dir.as_deref())?;
    let drafts = tags
        .iter()
        .map(|tag| tag.open(&token, &repo, &head, &declared))
        .collect::<Result<Vec<_>>>()?;
    if let Some((carrier, _)) = &declared {
        for (tag, opened) in tags.iter().zip(&drafts) {
            if tag.plan.tag == *carrier {
                for file in &files {
                    github::upload_to_release(&token, &repo, opened.id(), carrier, file)?;
                }
            }
        }
    }
    let lines = match run_uploads(uploads, registry_token, crates_io, scope.bootstrap) {
        Ok(lines) => lines,
        Err(stopped) => return Err(record_stop(&token, &repo, &tags, &drafts, stopped)),
    };
    let packages = already.into_iter().chain(lines).collect();
    let mut releases = Vec::with_capacity(tags.len());
    for (tag, opened) in tags.iter().zip(drafts) {
        releases.push(tag.publish(&token, &repo, &head, &declared, opened)?);
    }
    for tag in &tags {
        github::prove_tag(
            &token,
            &repo,
            &tag.plan.tag,
            &head,
            &github::TAG_PROOF_DELAYS,
        )?;
    }
    Ok(PublishRun {
        outcome: Outcome::Released,
        published: PublishOutcome { packages, releases },
        allowed,
        shortfalls: pending.shortfalls,
        proof: proof.rows(),
    })
}

/// The packages of `config` that `here` holds, with their plan entries.
fn selected_packages<'a>(
    config: &'a Config,
    planned: &'a PublishPlan,
    here: &pending::PendingCommit,
) -> Vec<(&'a PackageSpec, &'a PublishEntry)> {
    config
        .packages
        .iter()
        .zip(&planned.packages)
        .filter(|(_, entry)| {
            here.packages
                .iter()
                .any(|package| package.name == entry.name)
        })
        .collect()
}

/// The entries among `selected` at `0.0.0`, whose tags [`zero_tags`] renders.
fn selected_zero<'a>(selected: &[(&PackageSpec, &'a PublishEntry)]) -> Vec<&'a PublishEntry> {
    selected
        .iter()
        .map(|(_, entry)| *entry)
        .filter(|entry| bump::unreleased(&entry.version))
        .collect()
}

/// The release proof of `config`: each check `release_checks` names, against
/// the crates.io API in `crates_io`, with its owner's token for `trust`.
fn release_proof(
    config: &Config,
    root: &Path,
    crates_io: CratesIo<'_>,
    bootstrap: bool,
) -> Result<Proof> {
    registry_check::prove(
        config,
        root,
        crates_io.api,
        &config.release_checks,
        crates_io.owner_token,
        bootstrap,
    )
}

/// The uploads of each package at HEAD, and the lines of those already listed.
type Gathered<'a> = (Vec<(&'a PublishEntry, Vec<Upload>)>, Vec<PublishLine>);

/// The uploads of the packages at HEAD, and a line for each one the registry
/// already lists, which runs no upload again.
///
/// With `releases_only`, a package that still needs an upload fails the run
/// before any publisher starts.
fn gather_uploads<'a>(
    config: &Config,
    root: &Path,
    here: &pending::PendingCommit,
    selected: &[(&PackageSpec, &'a PublishEntry)],
    releases_only: bool,
    head: &str,
) -> Result<Gathered<'a>> {
    let mut uploads = Vec::new();
    let mut already = Vec::new();
    for (spec, entry) in selected {
        if !entry.argv.is_empty() && listed(here, &entry.name) {
            already.push(PublishLine {
                note: Some("already".to_owned()),
                ..PublishLine::from(*entry)
            });
            continue;
        }
        ensure!(
            !releases_only || entry.argv.is_empty(),
            "{name} {version} is not on its registry, and --releases-only starts no publisher; run verctl release without it at {head}",
            name = entry.name,
            version = entry.version,
        );
        let runs = if entry.argv.is_empty() {
            Vec::new()
        } else {
            publisher::uploads(config, spec, root, entry)?
        };
        uploads.push((*entry, runs));
    }
    Ok((uploads, already))
}

/// A run that found nothing to release.
fn nothing_to_release(allowed: Vec<Allowed>, shortfalls: Vec<pending::Shortfall>) -> PublishRun {
    PublishRun {
        outcome: Outcome::NothingToRelease,
        published: PublishOutcome {
            packages: Vec::new(),
            releases: Vec::new(),
        },
        allowed,
        shortfalls,
        proof: Vec::new(),
    }
}

/// Whether `pending` found the version of `name` already uploaded, so a run
/// that only has the Release or its assets left, such as a draft to publish,
/// runs no upload for it.
fn listed(here: &pending::PendingCommit, name: &str) -> bool {
    here.packages
        .iter()
        .any(|package| package.name == name && package.listed)
}

/// An upload that failed: the package it belongs to, the stop its draft
/// records, and the error the run returns.
struct Stopped {
    /// The package under whose tag the draft is.
    package: String,
    stop: Box<stop::Stop>,
    error: anyhow::Error,
}

/// Run every upload, and return each package's line, or the first upload that
/// failed.
///
/// A registry rate limit is recorded as its status with the time the
/// registry gave for the next attempt. Any other failure is recorded as the
/// first line of its error.
///
/// With `bootstrap`, an upload of a crate crates.io lacks creates it with the
/// registry token, and its line notes that.
fn run_uploads(
    uploads: Vec<(&PublishEntry, Vec<Upload>)>,
    registry_token: Option<&RegistryToken>,
    crates_io: CratesIo<'_>,
    bootstrap: bool,
) -> Result<Vec<PublishLine>, Stopped> {
    let mut packages = Vec::new();
    for (entry, uploads) in uploads {
        if entry.argv.is_empty() {
            packages.push(PublishLine::from(entry));
            continue;
        }
        for upload in uploads {
            let creating = bootstrap
                && creates(&upload, crates_io.api)
                    .map_err(|error| stopped(entry, &upload, error))?;
            let note = publish_checked(
                &upload,
                registry_token,
                crates_io.index,
                &registry::LISTING_DELAYS,
                creating,
            )
            .map_err(|error| stopped(entry, &upload, error))?;
            let note = match note {
                None if creating => Some(CREATED.to_owned()),
                note => note,
            };
            packages.push(PublishLine {
                name: upload.name,
                version: upload.version,
                via: entry.label.clone(),
                note,
            });
        }
    }
    Ok(packages)
}

/// The note on a crate a `--bootstrap` run created.
const CREATED: &str = "created with the registry token";

/// Whether `upload` creates its crate: it goes to crates.io, which has no
/// crate of that name.
fn creates(upload: &Upload, api: &str) -> Result<bool> {
    match &upload.check {
        RegistryCheck::CratesIo { name } => {
            Ok(registry::crates_io_user_owners(api, name)?.is_none())
        }
        RegistryCheck::Query { .. } | RegistryCheck::Unchecked => Ok(false),
    }
}

/// The stop of a failed `upload` of `entry`, and the error that failed it.
fn stopped(entry: &PublishEntry, upload: &Upload, error: anyhow::Error) -> Stopped {
    let stop = if let Some(limited) = error.downcast_ref::<RateLimited>() {
        stop::Stop::new(
            &upload.name,
            &upload.version,
            RATE_LIMIT,
            limited.limit.retry_after.clone(),
        )
    } else {
        let message = format!("{error:#}");
        let cause = message.lines().next().unwrap_or_default();
        stop::Stop::new(&upload.name, &upload.version, cause, None)
    };
    Stopped {
        package: entry.name.clone(),
        stop: Box::new(stop),
        error,
    }
}

/// `stopped`'s error, after its stop is written into the draft of its
/// package's tag, so `status` can name the cause.
/// A Release published before the run has no draft and gets no record.
/// A failure to write the record joins the error's context and never hides
/// the upload's error.
fn record_stop(
    token: &str,
    repo: &github::Repo,
    tags: &[TagRelease<'_>],
    drafts: &[Held],
    stopped: Stopped,
) -> anyhow::Error {
    let Some((tag, Held::Draft(id))) = tags.iter().zip(drafts).find(|(tag, _)| {
        tag.plan
            .packages
            .iter()
            .any(|entry| entry.name == stopped.package)
    }) else {
        return stopped.error;
    };
    let written = stop::write(&tag.notes, &stopped.stop)
        .and_then(|body| github::write_draft_body(token, repo, &tag.plan.tag, *id, &body));
    match written {
        Ok(()) => stopped.error,
        Err(failed) => stopped
            .error
            .context(format!("the upload failed, and {failed:#}")),
    }
}

/// The tag that carries the declared assets, and their file names.
/// `None` when no asset target is declared or `--skip-assets` leaves them out.
type DeclaredAssets = Option<(String, Vec<String>)>;

/// The declared assets, which belong to the first package's tag, as
/// [`assets::plan`] resolves it.
/// The names are `scope.declared_assets` when the plan gave any, so a run
/// declares what the plan scheduled, and otherwise every target this
/// commit's config names.
fn declared_assets(config: &Config, root: &Path, scope: &Scope) -> Result<DeclaredAssets> {
    if scope.skip_assets || config.assets.is_none() || config.packages.is_empty() {
        return Ok(None);
    }
    let planned = assets::plan(config, root)?;
    let names: Vec<String> = if scope.declared_assets.is_empty() {
        planned
            .matrix
            .include
            .iter()
            .map(|target| target.asset.clone())
            .collect()
    } else {
        scope.declared_assets.clone()
    };
    plain_names(&names)?;
    Ok((!names.is_empty()).then_some((planned.tag, names)))
}

/// Refuse every asset name that is not a plain file name: one with a
/// directory, `..`, or a root. A run joins each name to `--assets-dir`, so
/// such a name would read a file outside it.
fn plain_names(names: &[String]) -> Result<()> {
    let refused: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| Path::new(name).file_name() != Some(OsStr::new(name)))
        .collect();
    ensure!(
        refused.is_empty(),
        "an asset name must be a plain file name, with no directory, `..`, or root: {refused}; fix --declared-assets or the asset targets",
        refused = refused.join(", ")
    );
    Ok(())
}

/// The asset names the Release for `tag` must hold before it is published.
fn assets_of<'a>(declared: &'a DeclaredAssets, tag: &str) -> &'a [String] {
    match declared {
        Some((carrier, names)) if carrier == tag => names,
        Some(_) | None => &[],
    }
}

/// The tarballs a run uploads: each declared asset that the carrier tag's
/// draft, or its mutable Release published at HEAD, lacks, as a file in
/// `dir`.
/// An immutable published Release takes none, and a stale draft is deleted,
/// so all its assets come from `dir`.
/// It is checked before the run writes anything, so a tarball that is missing
/// fails the run before the draft exists.
fn asset_files(
    tags: &[TagRelease<'_>],
    declared: &DeclaredAssets,
    dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let Some((carrier, names)) = declared else {
        return Ok(Vec::new());
    };
    let Some(uploaded) = tags
        .iter()
        .find(|tag| tag.plan.tag == *carrier)
        .and_then(|tag| tag.found.uploaded())
    else {
        return Ok(Vec::new());
    };
    lacking(names, &uploaded, dir)
}

/// The files in `dir` for each of `names` that `uploaded` lacks.
///
/// With none lacking, `dir` is not needed. Otherwise every lacking name must
/// be a file in `dir`, and the error names each one that is not.
fn lacking(names: &[String], uploaded: &[&str], dir: Option<&Path>) -> Result<Vec<PathBuf>> {
    let wanted: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| !uploaded.contains(name))
        .collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let Some(dir) = dir else {
        bail!(
            "the release declares {wanted}, which its Release does not hold; pass --assets-dir with the tarballs release asset built, or --skip-assets to release without them",
            wanted = wanted.join(", ")
        );
    };
    let missing: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|name| !dir.join(name).is_file())
        .collect();
    ensure!(
        missing.is_empty(),
        "--assets-dir {dir} lacks {missing}, which the release declares; build them with release asset, or pass --skip-assets to release without them",
        dir = dir.display(),
        missing = missing.join(", ")
    );
    Ok(wanted.into_iter().map(|name| dir.join(name)).collect())
}

/// One tag of a run: its plan, the notes and stage its Release carries, and
/// what GitHub held for it before the run wrote anything.
struct TagRelease<'a> {
    plan: &'a TagPlan,
    notes: String,
    stage: github::Stage,
    found: github::Found,
}

/// A tag's Release once the run opened its draft.
enum Held {
    /// Published at HEAD before the run.
    Published {
        /// Its URL.
        url: String,
        /// Its id, which a mutable one takes the tarballs it lacks on.
        id: u64,
    },
    /// The id of the draft the run uploads to and publishes.
    Draft(u64),
}

impl Held {
    /// The id of the Release the run uploads the tarballs to.
    const fn id(&self) -> u64 {
        match self {
            Self::Published { id, .. } | Self::Draft(id) => *id,
        }
    }
}

impl<'a> TagRelease<'a> {
    /// Read GitHub for `plan`'s tag at `head`, with the CHANGELOG sections of
    /// its packages as the notes.
    /// A candidate's Release is a prerelease that never becomes the latest.
    /// The Releases list comes from `release_list`, which the plan already
    /// read.
    fn read(
        config: &Config,
        root: &Path,
        token: &str,
        repo: &github::Repo,
        plan: &'a TagPlan,
        head: &str,
        release_list: &github::ReleaseList,
    ) -> Result<Self> {
        let notes = release::notes_for(
            config,
            root,
            plan.packages
                .iter()
                .map(|entry| (entry.name.as_str(), entry.version.as_str())),
        );
        let stage = if plan
            .packages
            .iter()
            .any(|entry| publisher::prerelease_tag(&entry.version).is_some())
        {
            github::Stage::Candidate
        } else {
            github::Stage::Stable
        };
        let found = github::read_release(token, repo, &plan.tag, head, release_list)?;
        Ok(Self {
            plan,
            notes,
            stage,
            found,
        })
    }

    /// The Release this tag's draft becomes, at `head`, holding the assets
    /// `declared` gives this tag.
    fn release<'b>(
        &'b self,
        head: &'b str,
        declared: &'b DeclaredAssets,
    ) -> github::NewRelease<'b> {
        github::NewRelease {
            tag: &self.plan.tag,
            name: &self.plan.title,
            body: &self.notes,
            sha: head,
            stage: self.stage,
            assets: assets_of(declared, &self.plan.tag),
        }
    }

    /// Open the draft, or keep a Release published at HEAD as it is.
    fn open(
        &self,
        token: &str,
        repo: &github::Repo,
        head: &str,
        declared: &DeclaredAssets,
    ) -> Result<Held> {
        match &self.found {
            github::Found::Published(release) => Ok(Held::Published {
                url: release.url().to_owned(),
                id: release.id(),
            }),
            github::Found::Unpublished(draft) => {
                github::open_draft(token, repo, &self.release(head, declared), draft)
                    .map(Held::Draft)
            }
        }
    }

    /// Publish the draft, which creates the tag, and return the Release URL.
    fn publish(
        &self,
        token: &str,
        repo: &github::Repo,
        head: &str,
        declared: &DeclaredAssets,
        opened: Held,
    ) -> Result<String> {
        match opened {
            Held::Published { url, .. } => Ok(url),
            Held::Draft(id) => {
                github::publish_draft(token, repo, &self.release(head, declared), id)
            }
        }
    }
}

/// What a dry run would publish: every package not at `0.0.0`, a workspace
/// package as its members in publish order, and every tag.
/// It reads no registry listing for those.
/// A standalone package at `0.0.0` joins the plan as [`pending::pending`]
/// plans it: crates.io lists that version, or `bootstrap` is set and crates.io
/// lacks the crate. Only those packages are read from the crates.io index at
/// `crates_io`, and the dry run cannot tell whether such a package's tag
/// exists, which needs a GitHub token.
/// With every package at `0.0.0` that is left out, there is nothing to
/// release.
///
/// With something to release, it then runs the release proof as `release`
/// does, and its rows carry every failure beside the plan, so the run fails
/// having written nothing.
fn dry_run_plan(
    config: &Config,
    root: &Path,
    planned: &PublishPlan,
    declared: &DeclaredAssets,
    crates_io: CratesIo<'_>,
    bootstrap: bool,
) -> Result<PublishRun> {
    let mut packages = Vec::new();
    let mut zero = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
        if bump::unreleased(&entry.version) {
            if !pending::zero_planned(config, root, spec, entry, crates_io.index, bootstrap)? {
                continue;
            }
            zero.push(entry);
        }
        match publisher::workspace_members(config, spec, root)? {
            Some(members) => packages.extend(members.into_iter().map(|member| PublishLine {
                name: member.name,
                version: member.version,
                via: entry.label.clone(),
                note: None,
            })),
            None => packages.push(PublishLine::from(entry)),
        }
    }
    let releases = planned
        .tags
        .iter()
        .chain(&zero_tags(config, &zero)?)
        .map(|tag| {
            if assets_of(declared, &tag.tag).is_empty() {
                formatdoc!("would create {tag}", tag = tag.tag)
            } else {
                formatdoc!("would create draft {tag}", tag = tag.tag)
            }
        })
        .collect();
    let (outcome, proof) = if packages.is_empty() {
        (Outcome::NothingToRelease, Vec::new())
    } else {
        (
            Outcome::Released,
            release_proof(config, root, crates_io, bootstrap)?.rows(),
        )
    };
    Ok(PublishRun {
        outcome,
        published: PublishOutcome { packages, releases },
        allowed: Vec::new(),
        shortfalls: Vec::new(),
        proof,
    })
}

/// The pending commit at `head`.
///
/// The plan holds HEAD only, so a plan that names another commit is a bug in
/// the plan, and fails before anything is published.
fn at_head<'a>(pending: &'a pending::Pending, head: &str) -> Result<&'a pending::PendingCommit> {
    let Some(here) = pending.at(head) else {
        let named: Vec<&str> = pending
            .commits
            .iter()
            .map(|commit| commit.sha.as_str())
            .collect();
        bail!(
            "the release plan names {}, not HEAD {head}",
            named.join(", ")
        );
    };
    Ok(here)
}

/// Run one upload under its registry check, and return `already` when the
/// registry had the version.
///
/// The publisher and its registry `query` run with `registry_token`, under
/// the variable it names. A `creating` upload, one that creates its crate,
/// takes the token that creates crates where the token holds two
/// ([`RegistryToken::env_for`]).
/// No other command gets it.
///
/// With a check, the registry decides.
/// A listed version skips the publisher, and after the publisher runs the
/// version must appear within `delays`.
/// A publisher that fails with a version-exists message counts only when the
/// registry then lists the version.
/// That case is a publish the earlier check missed while the index lagged.
///
/// Without a check, a zero exit or that stderr is enough.
/// A publisher stopped by a registry rate limit fails with the time the
/// registry gives for the next attempt (see [`rate_limited`]).
pub(crate) fn publish_checked(
    upload: &Upload,
    registry_token: Option<&RegistryToken>,
    index: &str,
    delays: &[Duration],
    creating: bool,
) -> Result<Option<String>> {
    let already = || Some("already".to_owned());
    let check = &upload.check;
    let coord = format!("{}@{}", upload.name, upload.version);
    let env = registry_token.map(|token| token.env_for(creating));
    let env = env.as_slice();
    if !matches!(check, RegistryCheck::Unchecked)
        && check
            .lists(index, &upload.version, env)
            .with_context(|| coord.clone())?
    {
        return Ok(already());
    }
    let Err(error) = process::run_inherit_env(&upload.argv, env, PUBLISH_TIMEOUT) else {
        if !matches!(check, RegistryCheck::Unchecked) {
            registry::wait_listed(check, index, &coord, &upload.version, env, delays)?;
        }
        return Ok(None);
    };
    let message = format!("{error:#}");
    if already_published(&message) {
        if !matches!(check, RegistryCheck::Unchecked) {
            registry::wait_listed(check, index, &coord, &upload.version, env, delays)
                .with_context(|| message.clone())?;
        }
        return Ok(already());
    }
    if let Some(limit) = rate_limited(&message) {
        return Err(RateLimited { coord, limit }.into());
    }
    Err(error).context(coord)
}

/// The tag plans of the standalone packages at `0.0.0` that a run plans.
///
/// [`plan`] leaves such a package out of its tags, so a new package beside a
/// released one does not break a shared `v{version}` tag. Each package's tag
/// is rendered over that package alone, so another version never breaks the
/// template. Packages that render one tag share its plan.
///
/// # Errors
///
/// Returns an error when [`tags_for`] rejects a package.
pub(crate) fn zero_tags(config: &Config, entries: &[&PublishEntry]) -> Result<Vec<TagPlan>> {
    let mut plans: Vec<TagPlan> = Vec::new();
    for entry in entries {
        for plan in tags_for(config, slice::from_ref(*entry))? {
            match plans.iter_mut().find(|known| known.tag == plan.tag) {
                Some(known) => known.packages.extend(plan.packages),
                None => plans.push(plan),
            }
        }
    }
    Ok(plans)
}

/// The cause a stop record gives for a registry rate limit.
const RATE_LIMIT: &str = "429 Too Many Requests";

/// What a registry reported when it refused an upload for its rate limit.
#[derive(Debug)]
pub(crate) struct RateLimit {
    /// The time it gave after "try again after", or `None` when it gave the
    /// status alone.
    pub(crate) retry_after: Option<String>,
}

impl fmt::Display for RateLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.retry_after {
            Some(time) => write!(f, "try again after {time}"),
            None => f.write_str(RATE_LIMIT),
        }
    }
}

/// An upload the registry's rate limit stopped.
#[derive(Debug)]
pub(crate) struct RateLimited {
    /// The package and version, as `name@version`.
    coord: String,
    /// What the registry reported.
    pub(crate) limit: RateLimit,
}

impl fmt::Display for RateLimited {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{coord}: the registry's rate limit stopped the upload ({limit}). No tag exists until the draft Release is published, after the last upload. Rerun release then: it skips every crate the registry already lists.",
            coord = self.coord,
            limit = self.limit
        )
    }
}

impl Error for RateLimited {}

/// What a registry reported when it refused an upload for its rate limit, or
/// `None` when `message` is no rate limit.
///
/// crates.io answers with status 429 and "Please try again after <time> or
/// email help@crates.io".
/// The [rate limits] page of crates.io documents this reply.
///
/// [rate limits]: https://crates.io/docs/rate-limits
fn rate_limited(message: &str) -> Option<RateLimit> {
    let lower = message.to_ascii_lowercase();
    if !lower.contains("too many requests") {
        return None;
    }
    let marker = "try again after ";
    let Some(at) = lower.find(marker) else {
        return Some(RateLimit { retry_after: None });
    };
    let rest = &message[at + marker.len()..];
    let end = [" or email", "\n"]
        .iter()
        .filter_map(|stop| rest.find(stop))
        .min()
        .unwrap_or(rest.len());
    Some(RateLimit {
        retry_after: Some(rest[..end].trim().trim_end_matches('.').to_owned()),
    })
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

/// Remove a shallow clone's boundary before any history walk.
/// Verctl owns the fetch, so a depth-1 checkout still finds its version
/// commit.
///
/// # Errors
///
/// Returns an error when [`git::ensure_full_history`] fails.
pub fn full_history(root: &Path) -> Result<()> {
    match git::configured_default_branch() {
        Some(branch) => git::ensure_full_history(root, &branch, &git::FetchDefault::from_env()),
        None => Ok(()),
    }
}

/// The changelog, ancestry, and workflow proofs, which need no token. Each
/// finding the operator allowed comes back; any other refuses the run.
fn prove_version_commit(
    config: &Config,
    root: &Path,
    planned: &PublishPlan,
    allow: Allow,
) -> Result<Vec<Allowed>> {
    release::require_changelog_versions(
        config,
        root,
        planned
            .packages
            .iter()
            .filter(|entry| !bump::unreleased(&entry.version))
            .map(|entry| (entry.name.as_str(), entry.version.as_str())),
    )?;
    if let Some(branch) = git::configured_default_branch() {
        git::ensure_origin_head(root, &branch, &git::FetchDefault::from_env())?;
    }
    let mut allowed = Vec::new();
    for (check, found, allows) in [
        (
            Check::OffDefaultBranch,
            git::off_default_history(root)?,
            allow.off_default_branch,
        ),
        (
            Check::WorkflowChange,
            git::workflows_differ(root)?,
            allow.workflow_change,
        ),
    ] {
        let Some(reason) = found else { continue };
        ensure!(
            allows,
            "release refuses this commit: {reason}; pass {flag} to release anyway",
            flag = check.flag(),
        );
        allowed.push(Allowed { check, reason });
    }
    Ok(allowed)
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
///
/// # Errors
///
/// Returns an error when:
///
/// - `packages` is empty.
/// - A template without `{name}` meets more than one version.
/// - A template with `{name}` renders the same tag for two packages.
/// - The template cannot be rendered.
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
    use indoc::indoc;

    use super::{PublishEntry, already_published, tags_for};
    use crate::config::Config;

    mod head {
        use super::super::at_head;
        use crate::pending::{Pending, PendingCommit, PendingPackage};

        fn commit(sha: &str, name: &str, version: &str) -> PendingCommit {
            PendingCommit {
                sha: sha.into(),
                packages: vec![PendingPackage {
                    name: name.into(),
                    version: version.into(),
                    listed: false,
                }],
                declared: Vec::new(),
            }
        }

        /// A run at HEAD releases the plan's commit, whichever earlier commit
        /// set the versions in it.
        #[test]
        fn the_plan_at_head_is_released() {
            let pending = Pending {
                commits: vec![commit("ccc", "app", "0.2.1")],
                ..Pending::default()
            };
            let here = at_head(&pending, "ccc").unwrap();
            assert_eq!(here.packages[0].name, "app");
        }

        /// A plan that names a commit other than HEAD is a bug, and the error
        /// names both.
        #[test]
        fn a_plan_that_names_another_commit_fails_loudly() {
            let pending = Pending {
                commits: vec![commit("aaa", "core", "0.3.1")],
                ..Pending::default()
            };
            let error = at_head(&pending, "bbb").unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("names aaa, not HEAD bbb"), "{message}");
        }
    }

    /// Which tag a run drafts, and which assets it declares for it.
    mod declared {
        use std::fs;

        use indoc::indoc;

        use super::super::{Scope, assets_of, declared_assets};
        use crate::config::Config;

        fn demo(yaml: &str) -> (tempfile::TempDir, Config) {
            let dir = tempfile::TempDir::new().unwrap();
            fs::write(
                dir.path().join("Cargo.toml"),
                indoc! {r#"
                    [package]
                    name = "demo"
                    version = "1.2.0"
                "#},
            )
            .unwrap();
            (dir, Config::parse(yaml).unwrap())
        }

        const TARGETS: &str = indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
            assets:
              darwin-arm64: {}
              linux-x64: {}
        "};

        const NO_TARGETS: &str = indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
        "};

        /// The declared assets belong to the first package's tag. Another tag
        /// of the same run, and a run with no target, are published at once.
        #[test]
        fn only_the_carrier_tag_is_drafted() {
            let (dir, config) = demo(TARGETS);
            let declared = declared_assets(&config, dir.path(), &Scope::default()).unwrap();
            let names = [
                "demo_1.2.0_macos_arm64.tar.gz".to_owned(),
                "demo_1.2.0_linux_x64.tar.gz".to_owned(),
            ];
            assert_eq!(assets_of(&declared, "v1.2.0"), names);
            assert_eq!(assets_of(&declared, "other@1.2.0"), Vec::<String>::new());

            let (dir, config) = demo(NO_TARGETS);
            let none = declared_assets(&config, dir.path(), &Scope::default()).unwrap();
            assert_eq!(assets_of(&none, "v1.2.0"), Vec::<String>::new());
        }

        #[test]
        fn assets_skipped_declare_nothing() {
            let (dir, config) = demo(TARGETS);
            let scope = Scope {
                skip_assets: true,
                ..Scope::default()
            };
            let declared = declared_assets(&config, dir.path(), &scope).unwrap();
            assert_eq!(assets_of(&declared, "v1.2.0"), Vec::<String>::new());
        }

        /// The error for a run that declares `name` through
        /// `--declared-assets`.
        fn refused(name: &str) -> String {
            let (dir, config) = demo(TARGETS);
            let scope = Scope {
                declared_assets: vec!["demo_1.2.0_linux_x64.tar.gz".to_owned(), name.to_owned()],
                ..Scope::default()
            };
            let error = declared_assets(&config, dir.path(), &scope).unwrap_err();
            format!("{error:#}")
        }

        #[test]
        fn a_name_with_a_parent_directory_is_refused() {
            let message = refused("../demo.tar.gz");
            assert!(message.contains(": ../demo.tar.gz;"), "{message}");
        }

        #[test]
        fn a_name_with_a_directory_is_refused() {
            let message = refused("dist/demo.tar.gz");
            assert!(message.contains(": dist/demo.tar.gz;"), "{message}");
        }

        #[test]
        fn an_absolute_name_is_refused() {
            let message = refused("/tmp/demo.tar.gz");
            assert!(message.contains(": /tmp/demo.tar.gz;"), "{message}");
        }

        #[test]
        fn a_bare_parent_is_refused() {
            let message = refused("..");
            assert!(message.contains(": ..;"), "{message}");
            assert!(!message.contains("demo_1.2.0_linux_x64"), "{message}");
        }

        /// A leg declares the plan's set instead of the targets its own config
        /// names.
        #[test]
        fn the_plans_assets_replace_the_commits_own() {
            let (dir, config) = demo(TARGETS);
            let scope = Scope {
                declared_assets: vec!["demo_1.2.0_linux_x64.tar.gz".to_owned()],
                ..Scope::default()
            };
            let declared = declared_assets(&config, dir.path(), &scope).unwrap();
            assert_eq!(
                assets_of(&declared, "v1.2.0"),
                ["demo_1.2.0_linux_x64.tar.gz".to_owned()]
            );
        }
    }

    /// Which declared tarballs a run uploads, and from where.
    mod lacking {
        use std::fs;

        use super::super::lacking;

        fn names() -> Vec<String> {
            vec!["a.tar.gz".to_owned(), "b.tar.gz".to_owned()]
        }

        /// A draft that holds every declared tarball needs no directory.
        #[test]
        fn a_draft_with_every_asset_needs_no_directory() {
            let files = lacking(&names(), &["a.tar.gz", "b.tar.gz"], None).unwrap();
            assert!(files.is_empty(), "{files:?}");
        }

        /// Only the tarballs the draft lacks come from the directory.
        #[test]
        fn only_what_the_draft_lacks_is_uploaded() {
            let dir = tempfile::TempDir::new().unwrap();
            fs::write(dir.path().join("b.tar.gz"), "b").unwrap();
            let files = lacking(&names(), &["a.tar.gz"], Some(dir.path())).unwrap();
            assert_eq!(files, [dir.path().join("b.tar.gz")]);
        }

        #[test]
        fn no_directory_names_the_flags() {
            let error = lacking(&names(), &[], None).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("a.tar.gz, b.tar.gz"), "{message}");
            assert!(message.contains("--assets-dir"), "{message}");
            assert!(message.contains("--skip-assets"), "{message}");
        }

        #[test]
        fn a_missing_tarball_is_named_with_the_flag() {
            let dir = tempfile::TempDir::new().unwrap();
            fs::write(dir.path().join("a.tar.gz"), "a").unwrap();
            let error = lacking(&names(), &[], Some(dir.path())).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("--assets-dir"), "{message}");
            assert!(message.contains("lacks b.tar.gz,"), "{message}");
        }
    }

    #[cfg(unix)]
    mod checked {
        use std::fs;
        use std::path::Path;
        use std::time::Duration;

        use indoc::{formatdoc, indoc};
        use tempfile::TempDir;

        use super::super::{publish_checked, rate_limited};
        use crate::publisher::Upload;
        use crate::registry::{CRATES_IO_INDEX, RegistryCheck};
        use crate::registry_token::RegistryToken;

        const NO_WAIT: [Duration; 2] = [Duration::ZERO, Duration::ZERO];

        /// What crates.io prints for a new crate over its burst.
        const RATE_LIMITED: &str = indoc! {"
            error: failed to publish demo v0.4.6 to registry at https://crates.io

            Caused by:
              the remote server responded with an error (status 429 Too Many Requests): You have published too many new crates in a short period of time. Please try again after Sat, 19 Oct 2026 01:23:45 GMT or email help@crates.io to have your limit increased.
        "};

        /// The argv that runs `body` as a shell script written to `name`.
        ///
        /// The script runs through `sh` instead of being executed itself.
        /// Tests run in parallel, and a freshly written file can still be open
        /// in a child that another test forked.
        /// Executing it then fails with `ETXTBSY` (Text file busy).
        fn script(dir: &Path, name: &str, body: &str) -> Vec<String> {
            let path = dir.join(name);
            fs::write(&path, body).unwrap();
            vec!["sh".to_owned(), path.display().to_string()]
        }

        /// An upload under `check` whose publisher records that it ran,
        /// prints `stderr`, then exits with `code`.
        fn upload(dir: &Path, code: u8, stderr: &str, check: RegistryCheck) -> Upload {
            let ran = dir.join("published");
            let said = dir.join("stderr");
            fs::write(&said, stderr).unwrap();
            let argv = script(
                dir,
                "publisher",
                &formatdoc! {r#"
                    #!/bin/sh
                    touch "{ran}"
                    cat "{said}" >&2
                    exit {code}
                "#, ran = ran.display(), said = said.display()},
            );
            Upload {
                name: "demo".into(),
                version: "0.4.6".into(),
                manifest: dir.join("Cargo.toml"),
                argv,
                check,
            }
        }

        /// A query that prints the version from its `listed_after`-th call on.
        /// It counts calls in a file and exits non-zero before that.
        fn query(dir: &Path, listed_after: u32) -> RegistryCheck {
            let calls = dir.join("calls");
            let argv = script(
                dir,
                "query",
                &formatdoc! {r#"
                    #!/bin/sh
                    n=$(($(cat "{calls}" 2>/dev/null || echo 0) + 1))
                    echo "$n" > "{calls}"
                    if [ "$n" -ge {listed_after} ]; then
                      echo 0.4.6
                      exit 0
                    fi
                    exit 1
                "#, calls = calls.display()},
            );
            RegistryCheck::Query {
                argv,
                dir: dir.to_path_buf(),
            }
        }

        fn ran(dir: &Path) -> bool {
            dir.join("published").exists()
        }

        #[test]
        fn a_listed_version_skips_the_publisher() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 1);
            let note = publish_checked(
                &upload(dir.path(), 0, "", check),
                None,
                CRATES_IO_INDEX,
                &NO_WAIT,
                false,
            )
            .unwrap();
            assert_eq!(note.as_deref(), Some("already"));
            assert!(!ran(dir.path()));
        }

        #[test]
        fn a_publisher_that_exits_zero_without_publishing_fails() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 99);
            let error = publish_checked(
                &upload(dir.path(), 0, "", check),
                None,
                CRATES_IO_INDEX,
                &NO_WAIT,
                false,
            )
            .unwrap_err();
            assert!(ran(dir.path()));
            assert!(format!("{error:#}").contains("demo@0.4.6"), "{error:#}");
        }

        #[test]
        fn registry_metadata_that_lags_the_publish_passes() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 3);
            let note = publish_checked(
                &upload(dir.path(), 0, "", check),
                None,
                CRATES_IO_INDEX,
                &NO_WAIT,
                false,
            )
            .unwrap();
            assert_eq!(note, None);
            assert!(ran(dir.path()));
        }

        #[test]
        fn a_failed_publisher_fails_without_waiting() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 2);
            let error = publish_checked(
                &upload(dir.path(), 1, "", check),
                None,
                CRATES_IO_INDEX,
                &NO_WAIT,
                false,
            )
            .unwrap_err();
            assert!(format!("{error:#}").contains("demo@0.4.6"), "{error:#}");
        }

        #[test]
        fn without_a_check_a_zero_exit_publishes() {
            let dir = TempDir::new().unwrap();
            let upload = upload(dir.path(), 0, "", RegistryCheck::Unchecked);
            let note = publish_checked(&upload, None, CRATES_IO_INDEX, &NO_WAIT, false).unwrap();
            assert_eq!(note, None);
            assert!(ran(dir.path()));
        }

        /// The publisher and its registry query read the token from the
        /// variable the operator named.
        #[test]
        fn the_publisher_and_its_query_run_with_the_registry_token() {
            let dir = TempDir::new().unwrap();
            let seen = dir.path().join("seen");
            let queried = dir.path().join("queried");
            let argv = script(
                dir.path(),
                "publisher",
                &formatdoc! {r#"
                    #!/bin/sh
                    printf '%s' "$CARGO_REGISTRIES_DEMO_TOKEN" > "{seen}"
                "#, seen = seen.display()},
            );
            let check = RegistryCheck::Query {
                argv: script(
                    dir.path(),
                    "query",
                    &formatdoc! {r#"
                        #!/bin/sh
                        printf '%s' "$CARGO_REGISTRIES_DEMO_TOKEN" >> "{queried}"
                        if [ -f "{seen}" ]; then echo 0.4.6; exit 0; fi
                        exit 1
                    "#, queried = queried.display(), seen = seen.display()},
                ),
                dir: dir.path().to_path_buf(),
            };
            let upload = Upload {
                name: "demo".into(),
                version: "0.4.6".into(),
                manifest: dir.path().join("Cargo.toml"),
                argv,
                check,
            };
            let token =
                RegistryToken::resolve(Some("t0ken".into()), None, "CARGO_REGISTRIES_DEMO_TOKEN")
                    .unwrap();
            let note =
                publish_checked(&upload, token.as_ref(), CRATES_IO_INDEX, &NO_WAIT, false).unwrap();
            assert_eq!(note, None);
            assert_eq!(fs::read_to_string(&seen).unwrap(), "t0ken");
            assert_eq!(fs::read_to_string(&queried).unwrap(), "t0ken".repeat(2));
        }

        #[test]
        fn a_rate_limit_names_the_crate_and_the_retry_time() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 99);
            let upload = upload(dir.path(), 101, RATE_LIMITED, check);
            let error =
                publish_checked(&upload, None, CRATES_IO_INDEX, &NO_WAIT, false).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("demo@0.4.6"), "{message}");
            assert!(
                message.contains("try again after Sat, 19 Oct 2026 01:23:45 GMT)"),
                "{message}"
            );
            assert!(message.contains("No tag exists"), "{message}");
            assert!(
                message.contains("skips every crate the registry already lists"),
                "{message}"
            );
        }

        #[test]
        fn a_rate_limit_without_a_time_still_says_so() {
            assert_eq!(
                rate_limited("status 429 Too Many Requests")
                    .map(|limit| limit.to_string())
                    .as_deref(),
                Some("429 Too Many Requests")
            );
            assert!(rate_limited("error: token is required").is_none());
        }
    }

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
