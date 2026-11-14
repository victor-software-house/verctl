//! The release freeze and the enforcement tier.
//!
//! While the default branch holds a version whose release is incomplete,
//! `check registry` fails every other pull request and merge group, so HEAD
//! never moves past an unreleased version ([`check`]).
//! It first waits for a release workflow run at the default branch's tip, so
//! a pull request checked during a normal release passes.
//! A pull request labelled [`UNFREEZE_LABEL`] passes the freeze alone, for a
//! release that can never complete.
//!
//! Only a pull request, `pull_request_target` or `merge_group` event freezes
//! ([`Event`]).
//! Anywhere else the freeze reports the pending release and never fails.
//! `release` runs the `registry` proof without it: that run is the pending
//! release, or changes nothing.
//!
//! [`Tier`] names how far the branch's rules enforce the freeze, for `status`.

use std::path::Path;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::registry_token::NewCrates;
use crate::versions::{self, Skip};
use crate::{git, github, lookup, pending, release};

/// The label of a pull request that passes the freeze. It follows the
/// `tool: state` form of release-please's `autorelease: pending`.
pub const UNFREEZE_LABEL: &str = "verctl: unfreeze";

/// How long the wait for a running release sleeps between two reads of its
/// workflow runs.
const POLL: Duration = Duration::from_secs(30);

/// The Actions event of a run that the freeze fails: a pull request, or a
/// merge group, with what its payload holds about the label.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// A `pull_request` or `pull_request_target` event, and the labels its
    /// payload holds.
    PullRequest {
        /// The names of the labels on the pull request.
        labels: Vec<String>,
    },
    /// A `merge_group` event, and the pull request its head ref names, whose
    /// labels only the API serves.
    MergeGroup {
        /// The number in `gh-readonly-queue/<base>/pr-<n>-<sha>`, or `None`
        /// when the head ref has another shape.
        pull_request: Option<u64>,
    },
}

/// The part of an event payload the freeze reads.
#[derive(Deserialize)]
struct Payload {
    pull_request: Option<PullRequestPayload>,
    merge_group: Option<MergeGroupPayload>,
}

#[derive(Deserialize)]
struct PullRequestPayload {
    #[serde(default)]
    labels: Vec<LabelPayload>,
}

#[derive(Deserialize)]
struct LabelPayload {
    name: String,
}

#[derive(Deserialize)]
struct MergeGroupPayload {
    head_ref: String,
}

impl Event {
    /// The event of this Actions run, or `None` for any other event, or
    /// outside Actions. It reads `GITHUB_EVENT_NAME` and, for a freezing
    /// event, the payload at `GITHUB_EVENT_PATH`.
    ///
    /// # Errors
    ///
    /// Returns an error when the event is a freezing one and its payload is
    /// unset, unreadable, not JSON, or lacks the object the event names.
    pub fn from_env() -> Result<Option<Self>> {
        let Ok(name) = env::var("GITHUB_EVENT_NAME") else {
            return Ok(None);
        };
        if !matches!(
            name.as_str(),
            "pull_request" | "pull_request_target" | "merge_group"
        ) {
            return Ok(None);
        }
        let path = env::var("GITHUB_EVENT_PATH")
            .with_context(|| format!("a {name} event needs GITHUB_EVENT_PATH"))?;
        let raw =
            fs::read_to_string(&path).with_context(|| format!("read GITHUB_EVENT_PATH {path}"))?;
        let payload: Payload = serde_json::from_str(&raw)
            .with_context(|| format!("parse GITHUB_EVENT_PATH {path}"))?;
        if name == "merge_group" {
            let group = payload
                .merge_group
                .context("the merge_group event holds no merge_group")?;
            return Ok(Some(Self::MergeGroup {
                pull_request: queued_pull_request(&group.head_ref),
            }));
        }
        let pull = payload
            .pull_request
            .with_context(|| format!("the {name} event holds no pull_request"))?;
        Ok(Some(Self::PullRequest {
            labels: pull.labels.into_iter().map(|label| label.name).collect(),
        }))
    }

    /// Whether the pull request carries [`UNFREEZE_LABEL`]. A merge group
    /// reads the labels of its pull request through the API.
    ///
    /// # Errors
    ///
    /// Returns an error when the API cannot give a merge group's labels, as
    /// described for [`github::pull_labels`].
    fn unfreezes(&self, token: &str, repo: &github::Repo) -> Result<bool> {
        match self {
            Self::PullRequest { labels } => Ok(labels.iter().any(|label| label == UNFREEZE_LABEL)),
            Self::MergeGroup {
                pull_request: Some(number),
            } => Ok(github::pull_labels(token, repo, *number)?
                .iter()
                .any(|label| label == UNFREEZE_LABEL)),
            Self::MergeGroup { pull_request: None } => Ok(false),
        }
    }
}

/// The pull request number in a merge group's head ref,
/// `refs/heads/gh-readonly-queue/<base>/pr-<n>-<sha>`.
fn queued_pull_request(head_ref: &str) -> Option<u64> {
    let (_, entry) = head_ref
        .strip_prefix("refs/heads/gh-readonly-queue/")?
        .rsplit_once('/')?;
    entry.strip_prefix("pr-")?.split('-').next()?.parse().ok()
}

/// A package whose release at the default branch's version is incomplete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PendingRelease {
    /// The package name.
    pub name: String,
    /// The version the default branch holds, which HEAD holds too.
    pub version: String,
}

/// What the freeze does with the pending releases it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stance {
    /// The event is a pull request or merge group: each pending release fails
    /// the check.
    Enforced,
    /// Any other event: each pending release is a notice.
    Reported,
    /// The pull request carries [`UNFREEZE_LABEL`]: each pending release is a
    /// notice naming it.
    Unfrozen,
}

/// What the freeze found.
///
/// Not `#[non_exhaustive]`: the binary builds one in its presentation tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Freeze {
    /// No release of a version the default branch holds is incomplete.
    Clear,
    /// Some are.
    Held {
        /// What the check does with them.
        stance: Stance,
        /// Each one, in package order.
        pending: Vec<PendingRelease>,
    },
    /// The releases could not be searched: no token, or no answer from
    /// GitHub or a registry, on an event that never fails the freeze.
    NotChecked {
        /// Why.
        reason: String,
    },
}

impl Freeze {
    /// One message per pending release, when the freeze fails the check.
    #[must_use]
    pub fn failures(&self) -> Vec<String> {
        match self {
            Self::Held {
                stance: Stance::Enforced,
                pending,
            } => pending
                .iter()
                .map(|release| {
                    format!(
                        "`{name}@{version}` is not fully released on the default branch; finish or repair that release first, or label this pull request `{UNFREEZE_LABEL}` for a release that can never complete",
                        name = release.name,
                        version = release.version,
                    )
                })
                .collect(),
            Self::Clear
            | Self::NotChecked { .. }
            | Self::Held {
                stance: Stance::Reported | Stance::Unfrozen,
                ..
            } => Vec::new(),
        }
    }

    /// One notice per pending release the freeze only reports or the label
    /// passed, or the reason it did not run.
    #[must_use]
    pub fn notices(&self) -> Vec<String> {
        match self {
            Self::Held {
                stance: Stance::Reported,
                pending,
            } => pending
                .iter()
                .map(|release| {
                    format!(
                        "`{name}@{version}` is not fully released on the default branch; a pull request or merge group fails on it, and this run only reports it",
                        name = release.name,
                        version = release.version,
                    )
                })
                .collect(),
            Self::Held {
                stance: Stance::Unfrozen,
                pending,
            } => pending
                .iter()
                .map(|release| {
                    format!(
                        "`{name}@{version}` is not fully released on the default branch; the label `{UNFREEZE_LABEL}` passes the freeze",
                        name = release.name,
                        version = release.version,
                    )
                })
                .collect(),
            Self::NotChecked { reason } => {
                vec![format!("release freeze not checked: {reason}")]
            }
            Self::Clear
            | Self::Held {
                stance: Stance::Enforced,
                ..
            } => Vec::new(),
        }
    }
}

/// What the freeze reads besides the checkout.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Settings<'a> {
    /// The crates.io sparse index root, as [`pending::pending`] reads it.
    pub index: &'a str,
    /// How long to wait for a running release before deciding.
    pub wait: Duration,
}

impl<'a> Settings<'a> {
    /// Settings that read `index` and wait up to `wait`.
    #[must_use]
    pub const fn new(index: &'a str, wait: Duration) -> Self {
        Self { index, wait }
    }
}

/// The freeze of `config` at HEAD.
///
/// On a freezing [`Event`] it waits for a running release workflow run at the
/// default branch's tip, then searches for the pending releases and fails them
/// unless the pull request carries [`UNFREEZE_LABEL`]. Elsewhere it searches
/// without waiting, and reports.
///
/// The pending releases are the incomplete ones at HEAD
/// ([`pending::pending`]) whose version equals the one the merge-base with the
/// default branch holds. A Version PR, or a merge group whose earlier entry is
/// the Version PR, therefore does not freeze itself on its own new versions.
///
/// A config with no packages has nothing to release and is clear.
///
/// # Errors
///
/// Returns an error when a freezing event holds no `GITHUB_TOKEN`, when the
/// event payload cannot be read, when the wait runs out (`wait_for_release`),
/// or when the search or the label read fails. On any other event a lookup
/// GitHub or a registry gave no answer to is a [`Freeze::NotChecked`]
/// instead.
pub fn check(
    config: &Config,
    config_path: &Path,
    root: &Path,
    settings: &Settings<'_>,
) -> Result<Freeze> {
    if config.packages.is_empty() {
        return Ok(Freeze::Clear);
    }
    let event = Event::from_env()?;
    let Some(token) = release::env_token() else {
        ensure!(
            event.is_none(),
            "the release freeze reads the default branch's releases and workflow runs, and needs GITHUB_TOKEN"
        );
        return Ok(Freeze::NotChecked {
            reason: "no GITHUB_TOKEN or GH_TOKEN".to_owned(),
        });
    };
    let Some(event) = event else {
        return reported(config, config_path, root, &token, settings);
    };
    let repo = github::repo(root)?;
    let tip = git::default_tip(root, &git::release_branch_candidates())?;
    wait_for_release(&token, &repo, &config.release_workflow, &tip, settings.wait)?;
    let pending = held_releases(config, config_path, root, &token, settings.index)?;
    if pending.is_empty() {
        return Ok(Freeze::Clear);
    }
    let stance = if event.unfreezes(&token, &repo)? {
        Stance::Unfrozen
    } else {
        Stance::Enforced
    };
    Ok(Freeze::Held { stance, pending })
}

/// The freeze on an event that never fails it.
fn reported(
    config: &Config,
    config_path: &Path,
    root: &Path,
    token: &str,
    settings: &Settings<'_>,
) -> Result<Freeze> {
    match held_releases(config, config_path, root, token, settings.index) {
        Ok(pending) if pending.is_empty() => Ok(Freeze::Clear),
        Ok(pending) => Ok(Freeze::Held {
            stance: Stance::Reported,
            pending,
        }),
        Err(error) if lookup::is_unavailable(&error) => Ok(Freeze::NotChecked {
            reason: format!("{error:#}"),
        }),
        Err(error) => Err(error),
    }
}

/// Wait while a run of the workflow file `workflow` at commit `tip` has not
/// ended, reading its runs every 30 seconds, or sooner when less of `limit`
/// is left. It returns once no such run is active.
///
/// # Errors
///
/// Returns an error naming the run's URL and the limit when a run is still
/// active at the limit, or when the runs cannot be listed, as described for
/// [`github::active_run`].
fn wait_for_release(
    token: &str,
    repo: &github::Repo,
    workflow: &str,
    tip: &str,
    limit: Duration,
) -> Result<()> {
    let started = Instant::now();
    loop {
        let Some(run) = github::active_run(token, repo, workflow, tip)? else {
            return Ok(());
        };
        let left = limit.saturating_sub(started.elapsed());
        if left.is_zero() {
            bail!(
                "the release run {url} was still running when the {seconds}-second limit of --release-wait (VERCTL_RELEASE_WAIT) ran out; rerun this check once it ends, or raise the limit",
                url = run.url,
                seconds = limit.as_secs(),
            );
        }
        thread::sleep(POLL.min(left));
    }
}

/// The incomplete releases at HEAD, kept when the version equals the one the
/// merge-base with the default branch holds, the comparison `check versions`
/// makes.
fn held_releases(
    config: &Config,
    config_path: &Path,
    root: &Path,
    token: &str,
    index: &str,
) -> Result<Vec<PendingRelease>> {
    let planned = pending::pending(
        config,
        config_path,
        root,
        token,
        false,
        &[],
        None,
        index,
        &NewCrates::Off,
        &github::ReleaseList::default(),
    )?;
    let candidates = git::release_branch_candidates();
    let held = versions::report_with(root, config, Skip::None, &candidates, &candidates)?;
    Ok(planned
        .commits
        .into_iter()
        .flat_map(|commit| commit.packages)
        .filter(|package| {
            held.rows.iter().any(|row| {
                row.name == package.name
                    && row.released.as_deref() == Some(package.version.as_str())
            })
        })
        .map(|package| PendingRelease {
            name: package.name,
            version: package.version,
        })
        .collect())
}

/// How far the default branch's rules enforce the freeze.
///
/// Not `#[non_exhaustive]`: the binary labels every tier, so a new one fails
/// its build until it has a label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Tier {
    /// A `required_status_checks` rule and a merge queue apply: the job that
    /// runs `check registry` is required on the merge-group commit.
    MergeQueue,
    /// A `required_status_checks` rule applies and no merge queue does: the
    /// job is required on every pull request, up to date with the default
    /// branch.
    RulesetsWithoutQueue,
    /// No `required_status_checks` rule applies, so nothing makes `check
    /// registry` required: the operator runs `check versions registry` on the
    /// Version PR before merging, and the freeze is reported, not enforced.
    Neither,
    /// The rules could not be read.
    Unknown {
        /// Why: no token, or the answer GitHub gave.
        reason: String,
    },
}

impl Tier {
    /// The tier of the default branch's rules, from `GET
    /// /repos/{owner}/{repo}/rules/branches/{default}`.
    ///
    /// It never fails. Without a token, without a default branch name, or
    /// when GitHub refuses the read, as it does with a 403 or a 404 to a
    /// token that cannot read the rules, the tier is [`Tier::Unknown`] with
    /// the reason.
    #[must_use]
    pub fn read(root: &Path) -> Self {
        let Some(token) = release::env_token() else {
            return Self::Unknown {
                reason: "no GITHUB_TOKEN or GH_TOKEN".to_owned(),
            };
        };
        Self::of_branch(root, &token).unwrap_or_else(|error| Self::Unknown {
            reason: format!("{error:#}"),
        })
    }

    fn of_branch(root: &Path, token: &str) -> Result<Self> {
        let branch = git::configured_default_branch()
            .or_else(github::event_default_branch)
            .or_else(|| git::upstream_default_branch(root))
            .context(
                "cannot tell the default branch: set VERCTL_DEFAULT_BRANCH, or run `git remote set-head origin --auto`",
            )?;
        let repo = github::repo(root)?;
        Ok(Self::of_rules(&github::branch_rule_types(
            token, &repo, &branch,
        )?))
    }

    /// The tier of a branch whose rules have these `types`.
    ///
    /// Only a `required_status_checks` rule can make the job that runs
    /// `check registry` required, so a branch without one is
    /// [`Tier::Neither`], whatever other rules it holds. With that rule, a
    /// `merge_queue` rule makes the tier [`Tier::MergeQueue`].
    fn of_rules(types: &[String]) -> Self {
        let has = |wanted: &str| types.iter().any(|kind| kind == wanted);
        match (has("required_status_checks"), has("merge_queue")) {
            (true, true) => Self::MergeQueue,
            (true, false) => Self::RulesetsWithoutQueue,
            (false, _) => Self::Neither,
        }
    }

    /// The tier in a few words.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::MergeQueue => "merge queue".to_owned(),
            Self::RulesetsWithoutQueue => "rulesets without a queue".to_owned(),
            Self::Neither => "neither".to_owned(),
            Self::Unknown { reason } => format!("unknown ({reason})"),
        }
    }

    /// What the tier leaves unenforced, or `None` when the freeze is
    /// enforced or the tier is not known.
    #[must_use]
    pub const fn gap(&self) -> Option<&'static str> {
        match self {
            Self::Neither => Some("reported, not enforced"),
            Self::MergeQueue | Self::RulesetsWithoutQueue | Self::Unknown { .. } => None,
        }
    }
}
