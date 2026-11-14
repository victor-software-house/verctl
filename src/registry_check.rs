//! The `registry` and `trust` checks: what crates.io must hold before a
//! release uploads anything.
//!
//! `registry` needs no token for its crate proofs. For each crate the
//! repository publishes to crates.io, it proves that the crate exists and
//! lists every login `owners` expects. As evidence and not as proof, it
//! reports how the newest version reached crates.io.
//!
//! The command adds the release freeze of [`crate::freeze`] to the report
//! ([`RegistryReport::freeze`]); [`prove`], which `release` runs, leaves it
//! out, because that run is the release the freeze waits for.
//!
//! `trust` needs an owner's token in `VERCTL_OWNER_TOKEN`, because crates.io
//! serves trusted publisher entries to owners alone. It proves that an entry
//! names the repository and the workflow file `release_workflow` names.
//!
//! `release` runs the checks `release_checks` names as its proof ([`prove`]),
//! because registry state is all that can change between the merge and the
//! release.
//!
//! Both apply the existence proof to the crates the default branch's tip
//! holds. A crate that only the checked commit adds cannot be on crates.io
//! before it lands, so it passes with a notice to run `verctl release
//! bootstrap` once it does.
//!
//! `registry` also holds a standalone package that crates.io lacks to the
//! version where it starts, `0.0.0`, which `verctl release --new`
//! publishes. A member of a shared-version group first publishes at the
//! group's version and is exempt.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use toml_edit::{DocumentMut, Item};

use crate::config::{Config, ReleaseCheck};
use crate::freeze::Freeze;
use crate::github::{self, Repo};
use crate::publisher::Upload;
use crate::registry::{self, GithubConfig, RegistryCheck, TrustedPublishers, TrustedUpload};
use crate::registry_token::{NewCrates, OWNER};
use crate::{bump, git, publish, publisher};

/// A crate the repository uploads to crates.io.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Crate {
    /// The crate name, as its manifest spells it.
    pub name: String,
    /// Whether the default branch's tip holds a manifest at the same path
    /// that names this crate.
    pub held: bool,
    /// The logins its package expects among the owners, or `None` when the
    /// package has no owner proof ([`Config::owners_of`]).
    pub owners: Option<Vec<String>>,
    /// The version its upload sends.
    pub version: String,
    /// Whether the crate is its package's only upload, as opposed to a member
    /// of a workspace package, whose version its group shares.
    pub standalone: bool,
}

/// Every crate an `Upload` of `config` sends to crates.io, in upload order:
/// each member of a workspace package, and each other package whose
/// registry check reads crates.io. A package that uploads nothing is left
/// out.
///
/// The default branch resolves from `candidates` as [`git::default_tip`]
/// resolves it, and only when there is a crate to place.
///
/// # Errors
///
/// Returns an error when [`publish::plan`] or [`publisher::uploads`] fails,
/// when the default branch does not resolve, or when a manifest cannot be
/// read in the working tree or parsed at the default branch's tip.
pub fn crates(config: &Config, root: &Path, candidates: &[String]) -> Result<Vec<Crate>> {
    let planned = publish::plan(config, root)?;
    let mut found = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
        if entry.argv.is_empty() {
            continue;
        }
        let on_crates_io: Vec<(String, Upload)> = publisher::uploads(config, spec, root, entry)?
            .into_iter()
            .filter_map(|upload| match &upload.check {
                RegistryCheck::CratesIo { name } => Some((name.clone(), upload)),
                RegistryCheck::Query { .. } | RegistryCheck::Unchecked => None,
            })
            .collect();
        if on_crates_io.is_empty() {
            continue;
        }
        let standalone = publisher::workspace_members(config, spec, root)?.is_none();
        for (name, upload) in on_crates_io {
            found.push((name, upload, standalone, config.owners_of(spec)));
        }
    }
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let tip = git::default_tip(root, candidates)?;
    found
        .into_iter()
        .map(|(name, upload, standalone, owners)| {
            Ok(Crate {
                held: held_at(root, &tip, &upload.manifest, &name)?,
                name,
                owners: owners.map(<[String]>::to_vec),
                version: upload.version,
                standalone,
            })
        })
        .collect()
}

/// Whether commit `tip` holds `manifest`, a path in the working tree, and its
/// `package.name` there is `name`. A crate the pull request renames is not
/// held under its new name.
fn held_at(root: &Path, tip: &str, manifest: &Path, name: &str) -> Result<bool> {
    let manifest = manifest
        .canonicalize()
        .with_context(|| format!("resolve {}", manifest.display()))?;
    let Some(raw) = git::file_at(root, tip, &manifest)? else {
        return Ok(false);
    };
    let document = raw
        .parse::<DocumentMut>()
        .with_context(|| format!("{} at the default branch {tip}", manifest.display()))?;
    Ok(document
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(Item::as_str)
        == Some(name))
}

/// The crates of `config` against the default branch, fetched first as
/// `check versions` fetches it, and the repository they release from.
///
/// # Errors
///
/// Returns an error under the conditions of [`crates`], when the default
/// branch cannot be fetched, or when the repository does not resolve.
pub(crate) fn crates_from_env(config: &Config, root: &Path) -> Result<(Vec<Crate>, Option<Repo>)> {
    let fetch = git::FetchDefault::from_env();
    for branch in git::comparison_branches() {
        git::ensure_comparable(root, &branch, &fetch)?;
    }
    let crates = crates(config, root, &git::release_branch_candidates())?;
    if crates.is_empty() {
        return Ok((crates, None));
    }
    Ok((crates, Some(github::repo(root)?)))
}

/// `check registry`: each crate of `config` against the crates.io API at
/// `api`, which needs no token. It fetches and resolves the default branch as
/// `check versions` does, and reads the repository from
/// [`github::repo`]. The report holds no freeze: the command adds it with
/// [`crate::freeze::check`].
///
/// # Errors
///
/// Returns an error under the conditions of [`crates`] and [`prove_registry`],
/// when the default branch cannot be fetched, or when the repository does not
/// resolve.
pub fn registry(config: &Config, root: &Path, api: &str) -> Result<RegistryReport> {
    match crates_from_env(config, root)? {
        (crates, Some(repository)) => prove_registry(&crates, api, &repository, &NewCrates::Off),
        (_, None) => Ok(RegistryReport {
            crates: Vec::new(),
            freeze: None,
        }),
    }
}

/// The `registry` proof of each of `crates` against the crates.io API at
/// `api`, with `repository` as the source trusted publishing should name.
///
/// A crate crates.io lacks that `new_crates` allows, as a run of `release
/// --new` creates it, passes with a notice. The `0.0.0` rule still fails a
/// standalone package off `0.0.0`.
///
/// # Errors
///
/// Returns an error when an owners or versions request fails, as described
/// for [`registry::crates_io_user_owners`].
pub fn prove_registry(
    crates: &[Crate],
    api: &str,
    repository: &Repo,
    new_crates: &NewCrates,
) -> Result<RegistryReport> {
    let slug = slug(repository);
    let mut rows = Vec::with_capacity(crates.len());
    for listed in crates {
        let state = match registry::crates_io_user_owners(api, &listed.name)? {
            None if listed.standalone && !bump::unreleased(&listed.version) => {
                RegistryState::NotAtZero {
                    version: listed.version.clone(),
                }
            }
            None if new_crates.allows(&listed.name) => RegistryState::Creates,
            None if listed.held => RegistryState::Missing,
            None => RegistryState::New,
            Some(found) => {
                let lacking = listed.owners.as_ref().map_or_else(Vec::new, |expected| {
                    expected
                        .iter()
                        .filter(|login| {
                            !found.iter().any(|owner| owner.eq_ignore_ascii_case(login))
                        })
                        .cloned()
                        .collect()
                });
                RegistryState::Present {
                    expected: listed.owners.clone(),
                    owners: found,
                    lacking,
                    trusted: Evidence::of(
                        registry::crates_io_newest_trust(api, &listed.name)?,
                        &slug,
                    ),
                }
            }
        };
        rows.push(RegistryRow {
            name: listed.name.clone(),
            held: listed.held,
            state,
        });
    }
    Ok(RegistryReport {
        crates: rows,
        freeze: None,
    })
}

/// `check trust`: each crate of `config` against the trusted publisher
/// entries the crates.io API at `api` serves to `token`, an owner's token.
/// Without a token it sends no request and reads no branch, and the report
/// fails naming `VERCTL_OWNER_TOKEN`.
///
/// # Errors
///
/// Returns an error under the conditions of [`registry()`] and
/// [`prove_trust`].
pub fn trust(config: &Config, root: &Path, api: &str, token: Option<&str>) -> Result<TrustReport> {
    let Some(token) = token else {
        return Ok(TrustReport::Unset { variable: OWNER });
    };
    let (crates, repository) = crates_from_env(config, root)?;
    let Some(repository) = repository else {
        return Ok(TrustReport::Set {
            repository: None,
            workflow: config.release_workflow.clone(),
            crates: Vec::new(),
        });
    };
    prove_trust(
        &crates,
        api,
        token,
        &repository,
        &config.release_workflow,
        &NewCrates::Off,
    )
}

/// The release proof: each of `checks` against the crates.io API at `api`,
/// in the order [`ReleaseCheck::ALL`] declares them.
///
/// `registry` runs as `check registry` does, and a crate crates.io lacks that
/// `new_crates` allows, as a run of `release --new` creates it, passes with a
/// notice.
/// `trust` runs as `check trust`
/// does when `owner_token` holds an owner's token, and is skipped otherwise,
/// because a release under trusted publishing holds no such token. Both read
/// the crates and the default branch once. With nothing to run, it reads
/// nothing.
///
/// # Errors
///
/// Returns an error under the conditions of [`registry()`] and [`trust`].
pub fn prove(
    config: &Config,
    root: &Path,
    api: &str,
    checks: &[ReleaseCheck],
    owner_token: Option<&str>,
    new_crates: &NewCrates,
) -> Result<Proof> {
    let listed = |check| checks.contains(&check);
    let token = owner_token.filter(|_| listed(ReleaseCheck::Trust));
    let trust_skipped = listed(ReleaseCheck::Trust) && token.is_none();
    if !listed(ReleaseCheck::Registry) && token.is_none() {
        return Ok(Proof {
            registry: None,
            trust: None,
            trust_skipped,
        });
    }
    let (crates, repository) = crates_from_env(config, root)?;
    let registry = match (&repository, listed(ReleaseCheck::Registry)) {
        (Some(repository), true) => Some(prove_registry(&crates, api, repository, new_crates)?),
        (None, true) => Some(RegistryReport {
            crates: Vec::new(),
            freeze: None,
        }),
        (_, false) => None,
    };
    let trust = match (&repository, token) {
        (Some(repository), Some(token)) => Some(prove_trust(
            &crates,
            api,
            token,
            repository,
            &config.release_workflow,
            new_crates,
        )?),
        (None, Some(_)) => Some(TrustReport::Set {
            repository: None,
            workflow: config.release_workflow.clone(),
            crates: Vec::new(),
        }),
        (_, None) => None,
    };
    Ok(Proof {
        registry,
        trust,
        trust_skipped,
    })
}

/// What the release proof found.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Proof {
    /// The `registry` report, or `None` when `release_checks` leaves it out.
    pub registry: Option<RegistryReport>,
    /// The `trust` report, or `None` when it did not run.
    pub trust: Option<TrustReport>,
    /// Whether `release_checks` names `trust` and no owner's token was set,
    /// so it did not run.
    pub trust_skipped: bool,
}

impl Proof {
    /// The crates the `registry` check passes because the run creates them.
    #[must_use]
    pub fn created(&self) -> Vec<String> {
        self.registry.as_ref().map_or_else(Vec::new, |report| {
            report
                .crates
                .iter()
                .filter(|row| row.state == RegistryState::Creates)
                .map(|row| row.name.clone())
                .collect()
        })
    }

    /// One message per failed proof of either check.
    #[must_use]
    pub fn failures(&self) -> Vec<String> {
        self.rows()
            .into_iter()
            .filter_map(|row| match row {
                ProofRow::Failed { message, .. } => Some(message),
                ProofRow::Notice { .. } | ProofRow::Skipped { .. } => None,
            })
            .collect()
    }

    /// [`rows`](Self::rows) after a run that created `created`, by the name
    /// crates.io knows them.
    /// The run resolved every notice of each such crate, so no row of either
    /// check is left for it.
    #[must_use]
    pub fn rows_after(&self, created: &[String]) -> Vec<ProofRow> {
        let mut resolved = self.clone();
        if let Some(report) = &mut resolved.registry {
            report.crates.retain(|row| !created.contains(&row.name));
        }
        if let Some(TrustReport::Set { crates, .. }) = &mut resolved.trust {
            crates.retain(|row| !created.contains(&row.name));
        }
        resolved.rows()
    }

    /// Every row beyond a pass, check by check: each failure, then each
    /// notice, then `trust` when it was skipped.
    #[must_use]
    pub fn rows(&self) -> Vec<ProofRow> {
        let found = |check: ReleaseCheck, failures: Vec<String>, notices: Vec<String>| {
            failures
                .into_iter()
                .map(move |message| ProofRow::Failed { check, message })
                .chain(
                    notices
                        .into_iter()
                        .map(move |message| ProofRow::Notice { check, message }),
                )
        };
        let mut rows = Vec::new();
        if let Some(report) = &self.registry {
            rows.extend(found(
                ReleaseCheck::Registry,
                report.failures(),
                report.notices(),
            ));
        }
        if let Some(report) = &self.trust {
            rows.extend(found(
                ReleaseCheck::Trust,
                report.failures(),
                report.notices(),
            ));
        }
        if self.trust_skipped {
            rows.push(ProofRow::Skipped {
                check: ReleaseCheck::Trust,
                variable: OWNER,
            });
        }
        rows
    }
}

/// One row of the release proof beyond a pass, as the release report shows
/// it.
///
/// Not `#[non_exhaustive]`: the binary shows every row, so a new one fails
/// its build until it is shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "row", rename_all = "kebab-case")]
pub enum ProofRow {
    /// A proof that failed. A dry run reports it beside the plan; `release`
    /// stops before the draft instead.
    Failed {
        /// The check whose proof failed.
        check: ReleaseCheck,
        /// What failed, naming the crate and what to change.
        message: String,
    },
    /// A proof that passed without running, such as a crate that needs its
    /// bootstrap.
    Notice {
        /// The check that passed it.
        check: ReleaseCheck,
        /// What passed unproven.
        message: String,
    },
    /// A check `release_checks` names that did not run, because `variable`
    /// holds no owner's token.
    Skipped {
        /// The check that did not run.
        check: ReleaseCheck,
        /// The variable to set for it to run.
        variable: &'static str,
    },
}

/// The `trust` proof of each of `crates`: an entry must name `repository`
/// and `workflow`. A crate crates.io lacks is judged as `registry` judges
/// it, and its entries are not read. A crate whose entries crates.io refuses
/// to the token fails alone, and the other crates are still checked.
///
/// A crate crates.io lacks that `new_crates` allows passes with a notice to
/// add its entry once the run creates it.
///
/// # Errors
///
/// Returns an error when an owners request fails, as described for
/// [`registry::crates_io_user_owners`], or an entries request fails, as
/// described for [`registry::crates_io_github_configs`].
pub fn prove_trust(
    crates: &[Crate],
    api: &str,
    token: &str,
    repository: &Repo,
    workflow: &str,
    new_crates: &NewCrates,
) -> Result<TrustReport> {
    let mut rows = Vec::with_capacity(crates.len());
    for listed in crates {
        let state = match registry::crates_io_user_owners(api, &listed.name)? {
            None if new_crates.allows(&listed.name) => TrustState::Creates,
            None if listed.held => TrustState::Missing,
            None => TrustState::New,
            Some(_) => match registry::crates_io_github_configs(api, &listed.name, token)? {
                TrustedPublishers::Listed(entries)
                    if entries
                        .iter()
                        .any(|entry| names(entry, repository, workflow)) =>
                {
                    TrustState::Trusted
                }
                TrustedPublishers::Listed(entries) => TrustState::Untrusted { entries },
                TrustedPublishers::Refused { detail } => TrustState::Refused { detail },
            },
        };
        rows.push(TrustRow {
            name: listed.name.clone(),
            held: listed.held,
            state,
        });
    }
    Ok(TrustReport::Set {
        repository: Some(slug(repository)),
        workflow: workflow.to_owned(),
        crates: rows,
    })
}

/// Whether `entry` names `repository`, compared as GitHub compares names,
/// without case, and exactly `workflow`.
pub(crate) fn names(entry: &GithubConfig, repository: &Repo, workflow: &str) -> bool {
    entry
        .repository_owner
        .eq_ignore_ascii_case(&repository.owner)
        && entry.repository_name.eq_ignore_ascii_case(&repository.name)
        && entry.workflow_filename == workflow
}

/// `owner/name`.
pub(crate) fn slug(repository: &Repo) -> String {
    format!("{}/{}", repository.owner, repository.name)
}

/// The fix for a crate crates.io lacks while the default branch holds it.
fn bootstrap_fix(name: &str) -> String {
    format!(
        "{name}: missing from crates.io, though the default branch holds it; run `verctl release bootstrap {name}` at the default branch's tip"
    )
}

/// The failure for a never-published standalone package off `0.0.0`.
fn zero_fix(name: &str, version: &str) -> String {
    format!(
        "{name}: not on crates.io, and its version {version} is not 0.0.0; a never-published standalone package starts at 0.0.0, which `verctl release bootstrap {name}` publishes"
    )
}

/// The notice for a crate only the checked commit adds.
fn bootstrap_notice(name: &str) -> String {
    format!(
        "{name}: not on crates.io yet; it needs `verctl release bootstrap {name}` once it lands on the default branch"
    )
}

/// What `check registry` found: one row per crate, in upload order.
///
/// Not `#[non_exhaustive]`: the binary builds one in its presentation tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegistryReport {
    /// One row per crate.
    pub crates: Vec<RegistryRow>,
    /// The release freeze, or `None` when the run is the release itself or
    /// its dry run, which skips it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freeze: Option<Freeze>,
}

impl RegistryReport {
    /// One message per failed proof of a crate, naming it and what to
    /// change.
    #[must_use]
    pub fn crate_failures(&self) -> Vec<String> {
        self.crates
            .iter()
            .filter_map(RegistryRow::failure)
            .collect()
    }

    /// [`Self::crate_failures`], then one message per pending release the
    /// freeze fails.
    #[must_use]
    pub fn failures(&self) -> Vec<String> {
        let mut failures = self.crate_failures();
        failures.extend(self.freeze.iter().flat_map(Freeze::failures));
        failures
    }

    /// One message per proof that passed without running: a crate that
    /// needs its bootstrap, or a crate with no owner proof; then each notice
    /// of the freeze.
    #[must_use]
    pub fn notices(&self) -> Vec<String> {
        let mut notices: Vec<String> = self.crates.iter().filter_map(RegistryRow::notice).collect();
        notices.extend(self.freeze.iter().flat_map(Freeze::notices));
        notices
    }
}

/// One crate's row in the `registry` report.
///
/// Not `#[non_exhaustive]`: the binary builds one in its presentation tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegistryRow {
    /// The crate name.
    pub name: String,
    /// Whether the default branch's tip holds the crate.
    pub held: bool,
    /// What crates.io holds, and what the check made of it.
    #[serde(flatten)]
    pub state: RegistryState,
}

impl RegistryRow {
    /// Why this crate failed, or `None` when it passed.
    #[must_use]
    pub fn failure(&self) -> Option<String> {
        match &self.state {
            RegistryState::Missing => Some(bootstrap_fix(&self.name)),
            RegistryState::NotAtZero { version } => Some(zero_fix(&self.name, version)),
            RegistryState::Present {
                owners, lacking, ..
            } if !lacking.is_empty() => Some(format!(
                "{name}: lacks owner {lacking} on crates.io, which lists {found}; add it with `cargo owner --add`, or change owners in .ctl/ver.yaml",
                name = self.name,
                lacking = lacking.join(", "),
                found = if owners.is_empty() {
                    "no user owner".to_owned()
                } else {
                    owners.join(", ")
                },
            )),
            RegistryState::New | RegistryState::Creates | RegistryState::Present { .. } => None,
        }
    }

    /// What passed without being proven, or `None`.
    #[must_use]
    pub fn notice(&self) -> Option<String> {
        match &self.state {
            RegistryState::New => Some(bootstrap_notice(&self.name)),
            RegistryState::Creates => Some(format!(
                "{name}: crates.io lacks it; this run creates it with the registry token",
                name = self.name
            )),
            RegistryState::Present { expected: None, .. } => Some(format!(
                "{name}: owner proof skipped; set owners in .ctl/ver.yaml to prove them",
                name = self.name
            )),
            RegistryState::Missing
            | RegistryState::NotAtZero { .. }
            | RegistryState::Present { .. } => None,
        }
    }
}

/// What crates.io holds for a crate.
///
/// Not `#[non_exhaustive]`: the binary labels every state, so a new one fails
/// its build until it has a label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RegistryState {
    /// crates.io lacks a crate the default branch holds: the check fails.
    Missing,
    /// crates.io lacks a crate only the checked commit adds: the check
    /// passes, and the crate needs its bootstrap once it lands.
    New,
    /// crates.io lacks a standalone package whose version is not `0.0.0`,
    /// where a never-published package starts: the check fails, whether the
    /// default branch holds the crate or not.
    NotAtZero {
        /// The version the package holds.
        version: String,
    },
    /// crates.io lacks the crate and the run, started with `--new`,
    /// creates it with the registry token: the check passes with a notice.
    Creates,
    /// crates.io holds the crate.
    Present {
        /// The user logins crates.io lists as owners.
        owners: Vec<String>,
        /// The logins `owners` expects, or `None` when the owner proof is
        /// skipped.
        expected: Option<Vec<String>>,
        /// Each expected login crates.io does not list. The check fails when
        /// any is.
        lacking: Vec<String>,
        /// How the newest version reached crates.io, as evidence only.
        trusted: Evidence,
    },
}

/// How a crate's newest version reached crates.io, against the repository
/// the check runs in. It never fails the check.
///
/// Not `#[non_exhaustive]`, for the reason [`RegistryState`] gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "via", rename_all = "kebab-case")]
pub enum Evidence {
    /// Trusted publishing from this repository.
    ThisRepository {
        /// The `owner/name` crates.io recorded.
        repository: String,
    },
    /// Trusted publishing from another GitHub repository.
    OtherRepository {
        /// The `owner/name` crates.io recorded.
        repository: String,
    },
    /// Trusted publishing from a GitLab project.
    #[serde(rename = "gitlab")]
    GitLab {
        /// The project path crates.io recorded.
        project_path: String,
    },
    /// Not through trusted publishing.
    Untrusted,
}

impl Evidence {
    /// `upload` against `slug`, compared without case as GitHub compares
    /// names.
    fn of(upload: TrustedUpload, slug: &str) -> Self {
        match upload {
            TrustedUpload::GitHub { repository } if repository.eq_ignore_ascii_case(slug) => {
                Self::ThisRepository { repository }
            }
            TrustedUpload::GitHub { repository } => Self::OtherRepository { repository },
            TrustedUpload::GitLab { project_path } => Self::GitLab { project_path },
            TrustedUpload::Untrusted => Self::Untrusted,
        }
    }

    /// The evidence in a few words: `trusted from <repository>`, `gitlab`,
    /// or `not trusted`.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::ThisRepository { repository } | Self::OtherRepository { repository } => {
                format!("trusted from {repository}")
            }
            Self::GitLab { .. } => "gitlab".to_owned(),
            Self::Untrusted => "not trusted".to_owned(),
        }
    }
}

/// What `check trust` found.
///
/// Not `#[non_exhaustive]`, for the reason [`RegistryState`] gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "token", rename_all = "kebab-case")]
pub enum TrustReport {
    /// No owner's token: no entry was read, and the check fails.
    Unset {
        /// The variable to set.
        variable: &'static str,
    },
    /// The entries read with the token.
    Set {
        /// The `owner/name` an entry must name, or `None` when no crate
        /// publishes to crates.io.
        repository: Option<String>,
        /// The workflow file an entry must name: `release_workflow`.
        workflow: String,
        /// One row per crate, in upload order.
        crates: Vec<TrustRow>,
    },
}

impl TrustReport {
    /// One message per failed proof, naming its crate and what to change.
    #[must_use]
    pub fn failures(&self) -> Vec<String> {
        match self {
            Self::Unset { variable } => vec![format!(
                "{variable} is unset: set it to an owner's crates.io token with the trusted-publishing scope to prove the trusted publishers"
            )],
            Self::Set {
                repository,
                workflow,
                crates,
            } => crates
                .iter()
                .filter_map(|row| match &row.state {
                    TrustState::Missing => Some(bootstrap_fix(&row.name)),
                    TrustState::Untrusted { .. } => Some(format!(
                        "{name}: lacks a trusted publisher for {workflow} in {repository}; add one on crates.io that names this repository and workflow file",
                        name = row.name,
                        repository = repository.as_deref().unwrap_or("this repository"),
                    )),
                    TrustState::Refused { detail } => Some(format!(
                        "{name}: crates.io refused to list its trusted publishers ({detail}); the token's user must be an owner of the crate",
                        name = row.name,
                    )),
                    TrustState::New | TrustState::Creates | TrustState::Trusted => None,
                })
                .collect(),
        }
    }

    /// One notice per crate that needs its bootstrap, and one per crate the
    /// run creates, which then needs its entry.
    #[must_use]
    pub fn notices(&self) -> Vec<String> {
        match self {
            Self::Unset { .. } => Vec::new(),
            Self::Set { crates, .. } => crates
                .iter()
                .filter_map(|row| match row.state {
                    TrustState::New => Some(bootstrap_notice(&row.name)),
                    TrustState::Creates => Some(format!(
                        "{name}: crates.io lacks it; add its entry with `verctl release trust {name}` after this run creates it",
                        name = row.name
                    )),
                    TrustState::Trusted
                    | TrustState::Untrusted { .. }
                    | TrustState::Missing
                    | TrustState::Refused { .. } => None,
                })
                .collect(),
        }
    }
}

/// One crate's row in the `trust` report.
///
/// Not `#[non_exhaustive]`: the binary builds one in its presentation tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustRow {
    /// The crate name.
    pub name: String,
    /// Whether the default branch's tip holds the crate.
    pub held: bool,
    /// What the check found.
    #[serde(flatten)]
    pub state: TrustState,
}

/// What `check trust` found for one crate.
///
/// Not `#[non_exhaustive]`, for the reason [`RegistryState`] gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum TrustState {
    /// An entry names the repository and the workflow file.
    Trusted,
    /// No entry names both: the check fails.
    Untrusted {
        /// The entries crates.io holds for the crate.
        entries: Vec<GithubConfig>,
    },
    /// crates.io lacks a crate the default branch holds: the check fails.
    Missing,
    /// crates.io lacks a crate only the checked commit adds: the check
    /// passes, and the crate needs its bootstrap once it lands.
    New,
    /// crates.io lacks a crate the run, started with `--new`, creates:
    /// the check passes, and the crate needs its entry once the run creates
    /// it.
    Creates,
    /// crates.io refused to list the entries to the token, because its user
    /// does not own the crate: the check fails for this crate alone.
    Refused {
        /// crates.io's reason.
        detail: String,
    },
}
