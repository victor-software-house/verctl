//! Publish the versions already on HEAD, then open a GitHub Release.
//!
//! How each package is published comes from `[publishers.*]`. Cargo and
//! bun are stock recipes, not the only stacks. How tags are named comes
//! from `[tags].template` — see [`tags_for`].

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use ctl_core::formatdoc;
use serde::Serialize;

use crate::config::{Config, NAME_PLACEHOLDER};
use crate::publisher::Upload;
use crate::registry::{self, RegistryCheck};
use crate::{git, github, pending, process, publisher, release};

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

/// A package at its version on HEAD, with the command that uploads it.
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
/// # Errors
///
/// Returns an error when a package's version driver cannot be resolved, its
/// manifest cannot be read, or the driver finds no version in it. It also
/// fails when no publisher resolves for a package (see
/// [`publisher::resolve`]) or when [`tags_for`] rejects the packages.
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

/// Publish each package, then open a Release per tag. A dry run only plans.
///
/// This is [`run_pending`] with assets counted toward completeness.
///
/// # Errors
///
/// Returns an error when [`run_pending`] fails.
pub fn run(config: &Config, root: &Path, dry_run: bool) -> Result<PublishOutcome> {
    Ok(run_pending(config, root, dry_run, false)?.published)
}

/// Whether a publish run found anything to release.
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
/// A run on any other commit with an incomplete release fails before any side
/// effect and names the version commits, so a later commit can never tag one
/// that was never published. With `skip_assets`, declared assets do not count
/// toward completeness. A dry run plans every package and asks nothing.
///
/// # Errors
///
/// Returns an error when the plan cannot be resolved, a changelog lacks the
/// heading for a package version, HEAD is not on the default branch, no token
/// is set, a registry or GitHub lookup fails, HEAD is not the version commit
/// of an incomplete release, or a publisher, tag, or Release step fails.
pub fn run_pending(
    config: &Config,
    root: &Path,
    dry_run: bool,
    skip_assets: bool,
) -> Result<PublishRun> {
    let planned = plan(config, root)?;
    if dry_run {
        return dry_run_plan(config, root, &planned);
    }
    prove_version_commit(config, root, &planned)?;
    let token = release::resolve_token()?;
    let pending = pending::pending(config, root, &token, skip_assets)?;
    if pending.is_empty() {
        return Ok(PublishRun {
            outcome: Outcome::NothingToRelease,
            published: PublishOutcome {
                packages: Vec::new(),
                releases: Vec::new(),
            },
            waiting: Vec::new(),
        });
    }
    let head = git::head_sha(root)?;
    let (here, waiting) = at_head(&pending, &head)?;
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
    let mut packages = Vec::new();
    let mut shipped_packages = Vec::new();
    for (spec, entry) in selected {
        if entry.argv.is_empty() {
            packages.push(PublishLine::from(entry));
            continue;
        }
        for upload in publisher::uploads(config, spec, root, entry)? {
            let note = publish_checked(&upload, &registry::LISTING_DELAYS)?;
            if note.is_none() {
                shipped_packages.push(entry.name.clone());
            }
            packages.push(PublishLine {
                name: upload.name,
                version: upload.version,
                via: entry.label.clone(),
                note,
            });
        }
    }
    let repo = github::repo(root)?;
    let sha = git::head_sha(root)?;
    let mut releases = Vec::new();
    for tag in tags {
        let shipped = tag
            .packages
            .iter()
            .any(|entry| shipped_packages.contains(&entry.name));
        let notes = release::notes_for(
            config,
            root,
            tag.packages
                .iter()
                .map(|entry| (entry.name.as_str(), entry.version.as_str())),
        );
        let url =
            github::ensure_release(&token, &repo, &tag.tag, &tag.title, &notes, &sha, shipped)?;
        releases.push(url);
    }
    Ok(PublishRun {
        outcome: Outcome::Released,
        published: PublishOutcome { packages, releases },
        waiting,
    })
}

/// What a dry run would publish: every package, a workspace package as its
/// members in publish order, and every tag. It asks no registry.
fn dry_run_plan(config: &Config, root: &Path, planned: &PublishPlan) -> Result<PublishRun> {
    let releases = planned
        .tags
        .iter()
        .map(|tag| formatdoc!("would create {tag}", tag = tag.tag))
        .collect();
    let mut packages = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
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
    Ok(PublishRun {
        outcome: Outcome::Released,
        published: PublishOutcome { packages, releases },
        waiting: Vec::new(),
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
/// With a check, the registry decides: a listed version skips the publisher,
/// and after the publisher runs the version must appear within `delays`. A
/// publisher that fails saying the version exists counts only when the
/// registry then lists it, since that is a publish the earlier check missed
/// while the index lagged. Without a check, a zero exit or that stderr is
/// enough. A publisher stopped by a registry rate limit fails with the time
/// the registry gives for the next attempt (see [`rate_limited`]).
fn publish_checked(upload: &Upload, delays: &[Duration]) -> Result<Option<String>> {
    let already = || Some("already".to_owned());
    let check = &upload.check;
    let coord = format!("{}@{}", upload.name, upload.version);
    if !matches!(check, RegistryCheck::Unchecked)
        && check
            .lists(&upload.version)
            .with_context(|| coord.clone())?
    {
        return Ok(already());
    }
    let Err(error) = process::run_inherit(&upload.argv, PUBLISH_TIMEOUT) else {
        if !matches!(check, RegistryCheck::Unchecked) {
            registry::wait_listed(check, &coord, &upload.version, delays)?;
        }
        return Ok(None);
    };
    let message = format!("{error:#}");
    if already_published(&message) {
        if !matches!(check, RegistryCheck::Unchecked) {
            registry::wait_listed(check, &coord, &upload.version, delays)
                .with_context(|| message.clone())?;
        }
        return Ok(already());
    }
    if let Some(limit) = rate_limited(&message) {
        bail!(
            "{coord}: the registry's rate limit stopped the publish ({limit}). Nothing was tagged. Rerun the publish then: it skips every crate the registry already lists."
        );
    }
    Err(error).context(coord)
}

/// What a registry said when it refused an upload for its rate limit: the
/// time it gives after "try again after", or the status alone. `None` when
/// `message` is no rate limit.
///
/// crates.io answers with status 429 and "Please try again after <time> or
/// email help@crates.io": <https://crates.io/docs/rate-limits>.
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

fn prove_version_commit(config: &Config, root: &Path, planned: &PublishPlan) -> Result<()> {
    release::require_changelog_versions(
        config,
        root,
        planned
            .packages
            .iter()
            .map(|entry| (entry.name.as_str(), entry.version.as_str())),
    )?;
    if let Some(branch) = git::configured_default_branch() {
        git::ensure_origin_head(root, &branch, &git::FetchDefault::from_env())?;
    }
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
///
/// # Errors
///
/// Returns an error when `packages` is empty, when a template without
/// `{name}` meets more than one version, when a template with `{name}`
/// renders the same tag for two packages, or when the template cannot be
/// rendered.
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

        const NO_WAIT: [Duration; 2] = [Duration::ZERO, Duration::ZERO];

        /// What crates.io prints for a new crate over its burst.
        const RATE_LIMITED: &str = indoc! {"
            error: failed to publish demo v0.4.6 to registry at https://crates.io

            Caused by:
              the remote server responded with an error (status 429 Too Many Requests): You have published too many new crates in a short period of time. Please try again after Sat, 19 Oct 2026 01:23:45 GMT or email help@crates.io to have your limit increased.
        "};

        /// The argv that runs `body` as a shell script written to `name`.
        ///
        /// The script runs through `sh` rather than being executed itself:
        /// tests run in parallel, and a file just written can still be open
        /// in a child another test forked, so executing it fails with
        /// `ETXTBSY` (Text file busy).
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

        /// A query that prints the version from its `listed_after`-th call on,
        /// counting calls in a file, and exits non-zero before that.
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
            let note = publish_checked(&upload(dir.path(), 0, "", check), &NO_WAIT).unwrap();
            assert_eq!(note.as_deref(), Some("already"));
            assert!(!ran(dir.path()));
        }

        #[test]
        fn a_publisher_that_exits_zero_without_publishing_fails() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 99);
            let error = publish_checked(&upload(dir.path(), 0, "", check), &NO_WAIT).unwrap_err();
            assert!(ran(dir.path()));
            assert!(format!("{error:#}").contains("demo@0.4.6"), "{error:#}");
        }

        #[test]
        fn registry_metadata_that_lags_the_publish_passes() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 3);
            let note = publish_checked(&upload(dir.path(), 0, "", check), &NO_WAIT).unwrap();
            assert_eq!(note, None);
            assert!(ran(dir.path()));
        }

        #[test]
        fn a_failed_publisher_fails_without_waiting() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 2);
            let error = publish_checked(&upload(dir.path(), 1, "", check), &NO_WAIT).unwrap_err();
            assert!(format!("{error:#}").contains("demo@0.4.6"), "{error:#}");
        }

        #[test]
        fn without_a_check_a_zero_exit_publishes() {
            let dir = TempDir::new().unwrap();
            let upload = upload(dir.path(), 0, "", RegistryCheck::Unchecked);
            let note = publish_checked(&upload, &NO_WAIT).unwrap();
            assert_eq!(note, None);
            assert!(ran(dir.path()));
        }

        #[test]
        fn a_rate_limit_names_the_crate_and_the_retry_time() {
            let dir = TempDir::new().unwrap();
            let check = query(dir.path(), 99);
            let upload = upload(dir.path(), 101, RATE_LIMITED, check);
            let error = publish_checked(&upload, &NO_WAIT).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("demo@0.4.6"), "{message}");
            assert!(
                message.contains("try again after Sat, 19 Oct 2026 01:23:45 GMT)"),
                "{message}"
            );
            assert!(message.contains("Nothing was tagged"), "{message}");
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
