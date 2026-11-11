//! Publish the versions already on HEAD and open a GitHub Release.
//!
//! How each package is published comes from `[publishers.*]`.
//! Cargo and bun are stock recipes, and other stacks work as well.
//! How tags are named comes from `[tags].template`; see [`tags_for`].

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use ctl_core::formatdoc;
use serde::Serialize;

use crate::config::{Config, NAME_PLACEHOLDER};
use crate::publisher::Upload;
use crate::registry::{self, RegistryCheck};
use crate::registry_token::RegistryToken;
use crate::{bump, git, github, pending, process, publisher, release};

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
        let (label, noun, argv) = publisher::resolve(config, spec, root, &version)?;
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

/// Publish each package, then open a Release per tag. A dry run only plans.
///
/// This is [`run_pending`] with assets counted toward completeness and no
/// registry token, so each publisher finds its own credential.
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
        false,
        Allow::default(),
        None,
    )?
    .published)
}

/// The proofs a run may go past, each refused unless the operator allows it.
///
/// The one-version-commit proof has none.
/// Publishing past a skipped version leaves a gap in the registry that no
/// later run fills.
/// The fix is to release the older commit first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Allow {
    /// Publish when `.github/workflows/` at HEAD differs from the default
    /// branch's tip.
    pub workflow_change: bool,
    /// Publish from a commit the default branch does not contain.
    pub off_default_branch: bool,
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
    /// The run released the packages whose version commit is HEAD.
    Released,
}

/// What a publish run did, and what it left for another version commit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PublishRun {
    /// Whether there was anything to release.
    pub outcome: Outcome,
    /// The packages and Releases this run handled.
    pub published: PublishOutcome,
    /// Incomplete packages whose version commit is not HEAD.
    pub waiting: Vec<Waiting>,
    /// The proofs the operator allowed this run to go past.
    pub allowed: Vec<Allowed>,
}

/// An incomplete package left for the run at its own version commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Waiting {
    /// Package name.
    pub name: String,
    /// Version at HEAD.
    pub version: String,
    /// The version commit that releases it.
    pub commit: String,
}

/// Release the packages whose version commit is HEAD and whose release is
/// incomplete, and report nothing to release when every release is complete.
///
/// A tag whose packages all have a registry check, or no upload, gets its tag
/// and Release before any upload.
/// Any other tag gets them after its uploads (see `tags_before_upload`).
///
/// A run on any other commit with an incomplete release fails before any side
/// effect and names the version commits.
/// A later commit can therefore never tag a version that was never published.
///
/// With `skip_assets`, declared assets do not count toward completeness.
/// A package at `0.0.0` is never released and needs no changelog section.
/// A dry run plans every other package and queries no registry.
///
/// Before any publisher, tag, or Release call, the run proves the release
/// commit in three ways:
///
/// - The default branch contains it.
/// - Its `.github/workflows/` matches the tip of the default branch.
/// - Each package it releases has one version commit since its last release.
///
/// `allow` lets the run go past the first two.
/// [`PublishRun::allowed`] lists each one the run goes past.
///
/// # Errors
///
/// Returns an error when:
///
/// - The plan cannot be resolved.
/// - A changelog lacks the heading for a package version.
/// - A proof refuses or cannot run.
/// - No token is set.
/// - A registry or GitHub lookup fails.
/// - HEAD is not the version commit of an incomplete release.
/// - A publisher, tag, or Release step fails.
pub fn run_pending(
    config: &Config,
    config_path: &Path,
    root: &Path,
    dry_run: bool,
    skip_assets: bool,
    allow: Allow,
    registry_token: Option<&RegistryToken>,
) -> Result<PublishRun> {
    let planned = plan(config, root)?;
    if dry_run {
        return dry_run_plan(config, root, &planned);
    }
    full_history(root)?;
    let allowed = prove_version_commit(config, root, &planned, allow)?;
    let token = release::resolve_token()?;
    let pending = pending::pending(
        config,
        config_path,
        root,
        &token,
        skip_assets,
        registry_token,
    )?;
    if pending.is_empty() {
        return Ok(PublishRun {
            outcome: Outcome::NothingToRelease,
            published: PublishOutcome {
                packages: Vec::new(),
                releases: Vec::new(),
            },
            waiting: Vec::new(),
            allowed,
        });
    }
    let head = git::head_sha(root)?;
    let (here, waiting) = at_head(&pending, &head)?;
    refuse_skipped_versions(here, &waiting, &head)?;
    let selected: Vec<(&crate::config::PackageSpec, &PublishEntry)> = config
        .packages
        .iter()
        .zip(&planned.packages)
        .filter(|(_, entry)| {
            here.packages
                .iter()
                .any(|package| package.name == entry.name)
        })
        .collect();
    let tags: Vec<&TagPlan> = planned
        .tags
        .iter()
        .filter(|tag| {
            tag.packages.iter().any(|named| {
                here.packages
                    .iter()
                    .any(|package| package.name == named.name)
            })
        })
        .collect();
    let mut uploads = Vec::new();
    for (spec, entry) in &selected {
        let runs = if entry.argv.is_empty() {
            Vec::new()
        } else {
            publisher::uploads(config, spec, root, entry)?
        };
        uploads.push((*entry, runs));
    }
    let (before, after): (Vec<&TagPlan>, Vec<&TagPlan>) = tags
        .into_iter()
        .partition(|tag| tags_before_upload(tag, &uploads));
    let repo = github::repo(root)?;
    let (mut releases, refused) = open_early(config, root, &token, &repo, &before, &head)?;
    let rest = run_uploads(uploads, registry_token).and_then(|(packages, shipped_packages)| {
        for tag in after {
            let shipped = tag
                .packages
                .iter()
                .any(|entry| shipped_packages.contains(&entry.name));
            releases.push(open_release(
                config, root, &token, &repo, tag, &head, shipped,
            )?);
        }
        Ok(packages)
    });
    let packages = match (rest, refused) {
        (Ok(packages), None) => packages,
        (Ok(_), Some(early)) => return Err(early),
        (Err(later), early) => return Err(after_refusal(later, early)),
    };
    Ok(PublishRun {
        outcome: Outcome::Released,
        published: PublishOutcome { packages, releases },
        waiting,
        allowed,
    })
}

/// Open, at `head`, the Releases of the tags that exist before the uploads.
/// An existing tag must point at `head`.
///
/// When GitHub refuses a Release for a missing permission, the refusal is
/// returned beside the URLs and the run goes on.
/// The uploads still run, and a rerun with a token that has the permission
/// has only the Release left to do.
///
/// Any other failure, such as a tag that points at another commit, stops the
/// run here.
fn open_early(
    config: &Config,
    root: &Path,
    token: &str,
    repo: &github::Repo,
    tags: &[&TagPlan],
    head: &str,
) -> Result<(Vec<String>, Option<anyhow::Error>)> {
    let mut releases = Vec::new();
    let mut refused = None;
    for tag in tags {
        match open_release(config, root, token, repo, tag, head, true) {
            Ok(url) => releases.push(url),
            Err(error) if github::is_release_refused(&error) => {
                refused.get_or_insert(error);
            }
            Err(error) => return Err(after_refusal(error, refused)),
        }
    }
    Ok((releases, refused))
}

/// `error`, carrying a deferred Release refusal when there is one, so the
/// refusal is not lost behind a later failure.
fn after_refusal(error: anyhow::Error, refused: Option<anyhow::Error>) -> anyhow::Error {
    match refused {
        Some(early) => error.context(format!("{early:#}; then the run failed")),
        None => error,
    }
}

/// Run every upload, and return each package's line with the names of the
/// packages that shipped in this run.
fn run_uploads(
    uploads: Vec<(&PublishEntry, Vec<Upload>)>,
    registry_token: Option<&RegistryToken>,
) -> Result<(Vec<PublishLine>, Vec<String>)> {
    let mut packages = Vec::new();
    let mut shipped = Vec::new();
    for (entry, uploads) in uploads {
        if entry.argv.is_empty() {
            packages.push(PublishLine::from(entry));
            continue;
        }
        for upload in uploads {
            let note = publish_checked(&upload, registry_token, &registry::LISTING_DELAYS)?;
            if note.is_none() {
                shipped.push(entry.name.clone());
            }
            packages.push(PublishLine {
                name: upload.name,
                version: upload.version,
                via: entry.label.clone(),
                note,
            });
        }
    }
    Ok((packages, shipped))
}

/// Whether `tag` is created before the uploads: when no package under it
/// uploads without a registry check.
/// Completion then comes from the registry, so a tag created first claims
/// nothing.
/// The run that merged the Version PR creates it while HEAD is still the
/// default branch tip.
///
/// A rerun at that commit after a later merge changed `.github/workflows`
/// would otherwise need `workflows: write` to create the Release, which
/// `GITHUB_TOKEN` cannot hold.
///
/// An unchecked upload has only its tag as the record that it ran, so its tag
/// still follows the upload.
fn tags_before_upload(tag: &TagPlan, uploads: &[(&PublishEntry, Vec<Upload>)]) -> bool {
    !uploads.iter().any(|(entry, uploads)| {
        tag.packages.iter().any(|named| named.name == entry.name)
            && uploads
                .iter()
                .any(|upload| matches!(upload.check, RegistryCheck::Unchecked))
    })
}

/// Create the GitHub Release for `tag` at `sha`, or confirm it exists, with
/// the CHANGELOG sections of its packages as the body.
fn open_release(
    config: &Config,
    root: &Path,
    token: &str,
    repo: &github::Repo,
    tag: &TagPlan,
    sha: &str,
    shipped: bool,
) -> Result<String> {
    let notes = release::notes_for(
        config,
        root,
        tag.packages
            .iter()
            .map(|entry| (entry.name.as_str(), entry.version.as_str())),
    );
    let kind = if tag
        .packages
        .iter()
        .any(|entry| publisher::prerelease_tag(&entry.version).is_some())
    {
        github::ReleaseKind::Candidate
    } else {
        github::ReleaseKind::Stable
    };
    let release = github::NewRelease {
        tag: &tag.tag,
        name: &tag.title,
        body: &notes,
        sha,
        kind,
    };
    github::ensure_release(token, repo, &release, shipped)
}

/// What a dry run would publish: every package not at `0.0.0`, a workspace
/// package as its members in publish order, and every tag.
/// It queries no registry.
/// With every package at `0.0.0`, there is nothing to release.
fn dry_run_plan(config: &Config, root: &Path, planned: &PublishPlan) -> Result<PublishRun> {
    let releases = planned
        .tags
        .iter()
        .map(|tag| formatdoc!("would create {tag}", tag = tag.tag))
        .collect();
    let mut packages = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
        if bump::unreleased(&entry.version) {
            continue;
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
    let outcome = if packages.is_empty() {
        Outcome::NothingToRelease
    } else {
        Outcome::Released
    };
    Ok(PublishRun {
        outcome,
        published: PublishOutcome { packages, releases },
        waiting: Vec::new(),
        allowed: Vec::new(),
    })
}

/// The pending commit at `head`, and every incomplete package left for another
/// version commit. A `head` that releases nothing fails, naming the commits
/// that do, before anything is published.
fn at_head<'a>(
    pending: &'a pending::Pending,
    head: &str,
) -> Result<(&'a pending::PendingCommit, Vec<Waiting>)> {
    let waiting: Vec<Waiting> = pending
        .commits
        .iter()
        .filter(|commit| !commit.sha.eq_ignore_ascii_case(head))
        .flat_map(|commit| {
            commit.packages.iter().map(|package| Waiting {
                name: package.name.clone(),
                version: package.version.clone(),
                commit: commit.sha.clone(),
            })
        })
        .collect();
    let Some(here) = pending.at(head) else {
        let named: Vec<String> = waiting
            .iter()
            .map(|left| format!("{}@{} at {}", left.name, left.version, left.commit))
            .collect();
        bail!(
            "HEAD {head} is not the version commit of an incomplete release; publish at the version commit of {}",
            named.join(", ")
        );
    };
    Ok((here, waiting))
}

/// Run one upload under its registry check, and return `already` when the
/// registry had the version.
///
/// The publisher and its registry `query` run with `registry_token`, under
/// the variable it names.
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
fn publish_checked(
    upload: &Upload,
    registry_token: Option<&RegistryToken>,
    delays: &[Duration],
) -> Result<Option<String>> {
    let already = || Some("already".to_owned());
    let check = &upload.check;
    let coord = format!("{}@{}", upload.name, upload.version);
    let env = registry_token.map(RegistryToken::env);
    let env = env.as_slice();
    if !matches!(check, RegistryCheck::Unchecked)
        && check
            .lists(&upload.version, env)
            .with_context(|| coord.clone())?
    {
        return Ok(already());
    }
    let Err(error) = process::run_inherit_env(&upload.argv, env, PUBLISH_TIMEOUT) else {
        if !matches!(check, RegistryCheck::Unchecked) {
            registry::wait_listed(check, &coord, &upload.version, env, delays)?;
        }
        return Ok(None);
    };
    let message = format!("{error:#}");
    if already_published(&message) {
        if !matches!(check, RegistryCheck::Unchecked) {
            registry::wait_listed(check, &coord, &upload.version, env, delays)
                .with_context(|| message.clone())?;
        }
        return Ok(already());
    }
    if let Some(limit) = rate_limited(&message) {
        bail!(
            "{coord}: the registry's rate limit stopped the upload ({limit}). A package whose uploads all have a registry check is tagged before its upload, so its tag may exist. Rerun release then: it skips every crate the registry already lists."
        );
    }
    Err(error).context(coord)
}

/// What a registry reported when it refused an upload for its rate limit.
/// That is the time it gives after "try again after", or the status alone.
/// `None` when `message` is no rate limit.
///
/// crates.io answers with status 429 and "Please try again after <time> or
/// email help@crates.io".
/// The [rate limits] page of crates.io documents this reply.
///
/// [rate limits]: https://crates.io/docs/rate-limits
fn rate_limited(message: &str) -> Option<String> {
    let lower = message.to_ascii_lowercase();
    if !lower.contains("too many requests") {
        return None;
    }
    let marker = "try again after ";
    let Some(at) = lower.find(marker) else {
        return Some("429 Too Many Requests".to_owned());
    };
    let rest = &message[at + marker.len()..];
    let end = [" or email", "\n"]
        .iter()
        .filter_map(|stop| rest.find(stop))
        .min()
        .unwrap_or(rest.len());
    Some(format!(
        "try again after {}",
        rest[..end].trim().trim_end_matches('.')
    ))
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

/// Refuse a package HEAD releases while an earlier commit's version of it is
/// still unreleased.
/// Publishing this one would skip that version, and nothing would publish it
/// later.
/// `waiting` holds those versions, oldest first, because the plan lists each
/// one at the commit that set it.
fn refuse_skipped_versions(
    here: &pending::PendingCommit,
    waiting: &[Waiting],
    head: &str,
) -> Result<()> {
    for package in &here.packages {
        if let Some(skipped) = waiting.iter().find(|left| left.name == package.name) {
            bail!(
                "{name} {version} was never released: {sha} set it, and releasing {name} {current} at {head} would skip it; run `verctl status --github-output`, which lists {sha} first, and release that commit before this one",
                name = package.name,
                version = skipped.version,
                sha = skipped.commit,
                current = package.version,
            );
        }
    }
    Ok(())
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
        use super::super::{at_head, refuse_skipped_versions};
        use crate::pending::{Pending, PendingCommit, PendingPackage};

        fn pending() -> Pending {
            let package = |name: &str, version: &str| PendingPackage {
                name: name.into(),
                version: version.into(),
            };
            Pending {
                commits: vec![
                    PendingCommit {
                        sha: "aaa".into(),
                        packages: vec![package("core", "0.3.1")],
                    },
                    PendingCommit {
                        sha: "ccc".into(),
                        packages: vec![package("app", "0.2.1")],
                    },
                ],
                assets: Vec::new(),
            }
        }

        #[test]
        fn head_moved_past_the_release_names_the_version_commit() {
            let error = at_head(&pending(), "bbb").unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("core@0.3.1 at aaa"), "{message}");
            assert!(message.contains("app@0.2.1 at ccc"), "{message}");
        }

        #[test]
        fn another_package_pending_at_an_older_commit_waits() {
            let pending = pending();
            let (here, waiting) = at_head(&pending, "ccc").unwrap();
            assert_eq!(here.packages[0].name, "app");
            assert_eq!(waiting.len(), 1);
            assert_eq!(waiting[0].name, "core");
            assert_eq!(waiting[0].commit, "aaa");
        }

        /// Another package waiting is not a skip; the same package waiting
        /// at an older commit is, and the refusal names the way out.
        #[test]
        fn a_skipped_version_of_the_same_package_refuses() {
            let pending = pending();
            let (here, waiting) = at_head(&pending, "ccc").unwrap();
            refuse_skipped_versions(here, &waiting, "ccc").unwrap();

            let skipped = Pending {
                commits: vec![
                    PendingCommit {
                        sha: "aaa1111".into(),
                        packages: vec![PendingPackage {
                            name: "demo".into(),
                            version: "0.7.5".into(),
                        }],
                    },
                    PendingCommit {
                        sha: "bbb2222".into(),
                        packages: vec![PendingPackage {
                            name: "demo".into(),
                            version: "0.7.6".into(),
                        }],
                    },
                ],
                assets: Vec::new(),
            };
            let (here, waiting) = at_head(&skipped, "bbb2222").unwrap();
            let error = refuse_skipped_versions(here, &waiting, "bbb2222").unwrap_err();
            let message = format!("{error:#}");
            assert!(
                message.contains("demo 0.7.5 was never released"),
                "{message}"
            );
            assert!(message.contains("aaa1111 set it"), "{message}");
            assert!(
                message.contains("verctl status --github-output"),
                "{message}"
            );
        }
    }

    mod tag_order {
        use std::path::PathBuf;

        use super::super::{PublishEntry, TagPlan, tags_before_upload};
        use super::entry;
        use crate::publisher::Upload;
        use crate::registry::RegistryCheck;

        fn upload(name: &str, check: RegistryCheck) -> Upload {
            Upload {
                name: name.into(),
                version: "0.4.0".into(),
                manifest: PathBuf::from("Cargo.toml"),
                argv: vec!["true".into()],
                check,
            }
        }

        fn tag(packages: &[&PublishEntry]) -> TagPlan {
            TagPlan {
                tag: "v0.4.0".into(),
                title: "v0.4.0".into(),
                packages: packages.iter().map(|&entry| entry.clone()).collect(),
            }
        }

        #[test]
        fn a_crates_io_package_is_tagged_before_its_upload() {
            let demo = entry("demo", "0.4.0");
            let uploads = vec![(
                &demo,
                vec![upload(
                    "demo",
                    RegistryCheck::CratesIo {
                        name: "demo".into(),
                    },
                )],
            )];
            assert!(tags_before_upload(&tag(&[&demo]), &uploads));
        }

        #[test]
        fn a_release_only_package_is_tagged_first() {
            let mut app = entry("app", "0.4.0");
            app.argv.clear();
            assert!(tags_before_upload(&tag(&[&app]), &[(&app, Vec::new())]));
        }

        #[test]
        fn an_unchecked_upload_keeps_its_shared_tag_after_the_uploads() {
            let demo = entry("demo", "0.4.0");
            let tool = entry("tool", "0.4.0");
            let uploads = vec![
                (
                    &demo,
                    vec![upload(
                        "demo",
                        RegistryCheck::CratesIo {
                            name: "demo".into(),
                        },
                    )],
                ),
                (&tool, vec![upload("tool", RegistryCheck::Unchecked)]),
            ];
            assert!(!tags_before_upload(&tag(&[&demo, &tool]), &uploads));
            assert!(tags_before_upload(&tag(&[&demo]), &uploads));
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
        use crate::registry::RegistryCheck;
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
            let note = publish_checked(&upload(dir.path(), 0, "", check), None, &NO_WAIT).unwrap();
            assert_eq!(note.as_deref(), Some("already"));
            assert!(!ran(dir.path()));
        }

        #[test]
        fn a_publisher_that_exits_zero_without_publishing_fails() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 99);
            let error =
                publish_checked(&upload(dir.path(), 0, "", check), None, &NO_WAIT).unwrap_err();
            assert!(ran(dir.path()));
            assert!(format!("{error:#}").contains("demo@0.4.6"), "{error:#}");
        }

        #[test]
        fn registry_metadata_that_lags_the_publish_passes() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 3);
            let note = publish_checked(&upload(dir.path(), 0, "", check), None, &NO_WAIT).unwrap();
            assert_eq!(note, None);
            assert!(ran(dir.path()));
        }

        #[test]
        fn a_failed_publisher_fails_without_waiting() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 2);
            let error =
                publish_checked(&upload(dir.path(), 1, "", check), None, &NO_WAIT).unwrap_err();
            assert!(format!("{error:#}").contains("demo@0.4.6"), "{error:#}");
        }

        #[test]
        fn without_a_check_a_zero_exit_publishes() {
            let dir = TempDir::new().unwrap();
            let upload = upload(dir.path(), 0, "", RegistryCheck::Unchecked);
            let note = publish_checked(&upload, None, &NO_WAIT).unwrap();
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
            let note = publish_checked(&upload, token.as_ref(), &NO_WAIT).unwrap();
            assert_eq!(note, None);
            assert_eq!(fs::read_to_string(&seen).unwrap(), "t0ken");
            assert_eq!(fs::read_to_string(&queried).unwrap(), "t0ken".repeat(2));
        }

        #[test]
        fn a_rate_limit_names_the_crate_and_the_retry_time() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 99);
            let upload = upload(dir.path(), 101, RATE_LIMITED, check);
            let error = publish_checked(&upload, None, &NO_WAIT).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("demo@0.4.6"), "{message}");
            assert!(
                message.contains("try again after Sat, 19 Oct 2026 01:23:45 GMT)"),
                "{message}"
            );
            assert!(message.contains("its tag may exist"), "{message}");
            assert!(!message.contains("Nothing was tagged"), "{message}");
        }

        #[test]
        fn a_rate_limit_without_a_time_still_says_so() {
            assert_eq!(
                rate_limited("status 429 Too Many Requests").as_deref(),
                Some("429 Too Many Requests")
            );
            assert_eq!(rate_limited("error: token is required"), None);
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
