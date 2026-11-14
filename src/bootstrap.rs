//! `release bootstrap`: publish, with the operator's token, the crates
//! crates.io lacks.
//!
//! crates.io's trusted publishing cannot create a crate, and an entry can only
//! be added to a crate that exists. So the first version of a crate goes up
//! with a token, from the default branch's tip, at the version the default
//! branch holds. Each published standalone package then gets its tag, and
//! [`crate::trust`] adds the trusted publisher entry for every crate the run
//! published, so no crate is left without one.
//!
//! A member of a shared-version group gets no tag, because its tag belongs to
//! the group's release.

use std::path::Path;
use std::{fmt, slice, thread};

use anyhow::{Result, bail, ensure};

use crate::config::Config;
use crate::first_publish::{Row, State, select};
use crate::publish::{self, RateLimited};
use crate::registry::{self, CratesIo};
use crate::registry_check::{Planned, planned_from_env};
use crate::registry_token::{self, RegistryToken};
use crate::{git, github, http_date, trust};

/// How often one crate's upload waits out a rate limit before it fails.
const MAX_WAITS: usize = 10;

/// What `release bootstrap` runs with.
#[non_exhaustive]
pub struct Request<'a> {
    /// The crates to publish, or none for every crate crates.io lacks.
    pub names: &'a [String],
    /// Whether to list what it would publish and tag, and write nothing.
    pub dry_run: bool,
    /// The crates.io index and API, and the owner's token for the trusted
    /// publisher entries.
    pub crates_io: CratesIo<'a>,
    /// The operator's token, under the variable the publisher reads it from.
    pub registry_token: Option<&'a RegistryToken>,
    /// A GitHub token, which tags a standalone package.
    pub github_token: Option<&'a str>,
}

impl<'a> Request<'a> {
    /// A request for `names`, or for every crate crates.io lacks when it is
    /// empty.
    #[must_use]
    pub const fn new(
        names: &'a [String],
        dry_run: bool,
        crates_io: CratesIo<'a>,
        registry_token: Option<&'a RegistryToken>,
        github_token: Option<&'a str>,
    ) -> Self {
        Self {
            names,
            dry_run,
            crates_io,
            registry_token,
            github_token,
        }
    }
}

impl fmt::Debug for Request<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("names", &self.names)
            .field("dry_run", &self.dry_run)
            .field("crates_io", &self.crates_io)
            .field("registry_token", &self.registry_token)
            .field("github_token", &self.github_token.map(|_| "<redacted>"))
            .finish()
    }
}

/// Publish each crate `request` selects that crates.io lacks, tag each
/// published standalone package, and add the trusted publisher entry of each
/// published crate. Under `--dry-run` it lists what it would publish and tag,
/// and reads only crates.io and git.
///
/// A crate crates.io lists gets an `already published` row when `names` names
/// it, and no row otherwise. An upload that fails stops the uploads, and the
/// crates published before it still get their tags and entries.
///
/// # Errors
///
/// Returns an error, before any upload, when a token the run needs is unset.
/// It also returns one when the default branch cannot be fetched, when
/// `request.names` holds a crate the repository does not upload, or when HEAD
/// is not the default branch's tip.
pub fn run(config: &Config, root: &Path, request: &Request<'_>) -> Result<Vec<Row>> {
    if !request.dry_run {
        require_tokens(request)?;
    }
    let (planned, repository) = planned_from_env(config, root)?;
    let selected = select(&planned, request.names)?;
    let Some(repository) = repository else {
        return Ok(Vec::new());
    };
    let tip = at_the_tip(root)?;
    let mut rows = Vec::new();
    let mut lacking = Vec::new();
    for crate_ in selected {
        if registry::crates_io_user_owners(request.crates_io.api, &crate_.listed.name)?.is_some() {
            if !request.names.is_empty() {
                rows.push(versioned(crate_, State::AlreadyPublished, None));
            }
        } else {
            lacking.push(crate_);
        }
    }
    if request.dry_run {
        for crate_ in lacking {
            rows.push(versioned(crate_, State::WouldPublish, None));
            for tag in tags(config, crate_)? {
                rows.push(versioned(crate_, State::WouldTag { tag }, None));
            }
        }
        return Ok(rows);
    }
    ensure!(
        request.github_token.is_some() || lacking.iter().all(|crate_| !crate_.listed.standalone),
        "release bootstrap tags a standalone package and needs GITHUB_TOKEN or GH_TOKEN, which is unset"
    );
    let published = publish_all(&lacking, request, &mut rows);
    if let Some(token) = request.github_token {
        for crate_ in published.iter().filter(|crate_| crate_.listed.standalone) {
            rows.extend(tag(config, crate_, token, &repository, &tip));
        }
    }
    let names: Vec<&str> = published
        .iter()
        .map(|crate_| crate_.listed.name.as_str())
        .collect();
    if let Some(owner_token) = request.crates_io.owner_token {
        for row in trust::add(
            &names,
            request.crates_io.api,
            owner_token,
            &repository,
            &config.release_workflow,
            false,
        ) {
            let version = published
                .iter()
                .find(|crate_| crate_.listed.name == row.name)
                .map(|crate_| crate_.listed.version.clone());
            rows.push(Row { version, ..row });
        }
    }
    Ok(rows)
}

/// `crate_` as a row with its version, and `note`.
fn versioned(crate_: &Planned, state: State, note: Option<String>) -> Row {
    Row {
        name: crate_.listed.name.clone(),
        version: Some(crate_.listed.version.clone()),
        state,
        note,
    }
}

/// Fail naming each token a real run lacks: the operator's token to publish,
/// and the owner's token for the entries, so no crate is created without one.
fn require_tokens(request: &Request<'_>) -> Result<()> {
    let mut missing = Vec::new();
    if request.registry_token.is_none() {
        missing.push(format!(
            "{} is unset: set it to the crates.io token that publishes new crates",
            registry_token::SECRET
        ));
    }
    if request.crates_io.owner_token.is_none() {
        missing.push(format!(
            "{} is unset: set it to an owner's crates.io token with the trusted-publishing scope, which adds each crate's entry",
            registry_token::OWNER
        ));
    }
    ensure!(
        missing.is_empty(),
        "release bootstrap publishes and registers nothing until each token is set: {}",
        missing.join("; ")
    );
    Ok(())
}

/// Prove HEAD is the commit at the default branch's tip, which the caller has
/// fetched, and return it. The version `release bootstrap` publishes is the
/// one the default branch holds.
fn at_the_tip(root: &Path) -> Result<String> {
    let head = git::head_sha(root)?;
    let tip = git::default_tip(root, &git::release_branch_candidates())?;
    if !head.eq_ignore_ascii_case(&tip) {
        bail!(
            "HEAD {head} is not the default branch's tip {tip}; release bootstrap publishes the version the default branch holds, so check out {tip} first"
        );
    }
    Ok(tip)
}

/// The tags `crate_`'s package gets, rendered by `[tags].template`.
fn tags(config: &Config, crate_: &Planned) -> Result<Vec<String>> {
    if !crate_.listed.standalone {
        return Ok(Vec::new());
    }
    Ok(publish::tags_for(config, slice::from_ref(&crate_.entry))?
        .into_iter()
        .map(|plan| plan.tag)
        .collect())
}

/// Upload each of `lacking` in order, pushing a row for each, and return the
/// crates published. The first upload that fails ends the run of uploads.
fn publish_all<'a>(
    lacking: &[&'a Planned],
    request: &Request<'_>,
    rows: &mut Vec<Row>,
) -> Vec<&'a Planned> {
    let mut published = Vec::new();
    for crate_ in lacking {
        let (outcome, note) = upload(crate_, request);
        match outcome {
            Ok(state) => {
                if state == State::Published {
                    published.push(*crate_);
                }
                rows.push(versioned(crate_, state, note));
            }
            Err(error) => {
                let message = format!("{}: {error:#}", crate_.listed.name);
                rows.push(versioned(crate_, State::Failed { message }, note));
                break;
            }
        }
    }
    published
}

/// Run `crate_`'s upload, and run it again after each rate limit once the
/// time crates.io gave has passed, at most [`MAX_WAITS`] times. The note
/// names each wait. A limit with no time that parses fails as `release` does.
fn upload(crate_: &Planned, request: &Request<'_>) -> (Result<State>, Option<String>) {
    let mut waited: Vec<String> = Vec::new();
    let note = |waited: &[String]| {
        (!waited.is_empty()).then(|| {
            waited
                .iter()
                .map(|time| format!("waited until {time}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
    };
    loop {
        let error = match publish::publish_checked(
            &crate_.upload,
            request.registry_token,
            request.crates_io.index,
            &registry::LISTING_DELAYS,
        ) {
            Ok(Some(_)) => return (Ok(State::AlreadyPublished), note(&waited)),
            Ok(None) => return (Ok(State::Published), note(&waited)),
            Err(error) => error,
        };
        let limit = error
            .downcast_ref::<RateLimited>()
            .and_then(|limited| limited.limit.retry_after.as_deref())
            .filter(|_| waited.len() < MAX_WAITS)
            .and_then(|time| Some((time.to_owned(), http_date::wait(time)?)));
        let Some((time, wait)) = limit else {
            return (Err(error), note(&waited));
        };
        thread::sleep(wait);
        waited.push(time);
    }
}

/// The rows for `crate_`'s tags, each created at `tip` or found there.
fn tag(
    config: &Config,
    crate_: &Planned,
    token: &str,
    repository: &github::Repo,
    tip: &str,
) -> Vec<Row> {
    let name = &crate_.listed.name;
    let created = tags(config, crate_).and_then(|tags| {
        tags.into_iter()
            .map(|tag| {
                github::ensure_tag(token, repository, &tag, tip)?;
                Ok(tag)
            })
            .collect::<Result<Vec<_>>>()
    });
    match created {
        Ok(tags) => tags
            .into_iter()
            .map(|tag| versioned(crate_, State::Tagged { tag }, None))
            .collect(),
        Err(error) => vec![Row::failed(name, format!("{name}: {error:#}"))],
    }
}
