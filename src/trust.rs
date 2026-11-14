//! `release trust`: add this repository's trusted publisher entry to each
//! crate that has none.
//!
//! crates.io accepts duplicate entries, up to 5 per crate, so a create without
//! the list first would not be idempotent. The step lists each crate's
//! entries and adds one only when none names the repository and the workflow
//! file `release_workflow` names, the entry `check trust` proves. It never
//! edits or deletes another entry.

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::first_publish::{Row, State, select};
use crate::github::Repo;
use crate::registry::{self, Added, CratesIo, TrustedPublishers};
use crate::registry_check::{crates_from_env, names, slug};
use crate::registry_token::OWNER;

/// How many trusted publisher entries crates.io allows one crate.
const LIMIT: usize = 5;

/// What `release trust` runs with.
#[derive(Debug)]
#[non_exhaustive]
pub struct Request<'a> {
    /// The crates to handle, or none for every crate the repository uploads
    /// to crates.io.
    pub names: &'a [String],
    /// Whether to list the entries it would add and send no `POST`.
    pub dry_run: bool,
    /// The crates.io API and the owner's token. The index is unused.
    pub crates_io: CratesIo<'a>,
}

impl<'a> Request<'a> {
    /// A request for `names`, or for every crate when it is empty.
    #[must_use]
    pub const fn new(names: &'a [String], dry_run: bool, crates_io: CratesIo<'a>) -> Self {
        Self {
            names,
            dry_run,
            crates_io,
        }
    }
}

/// Add the entry to each crate `request` selects, one row each. A crate that
/// fails does not stop the others.
///
/// The repository comes from [`crate::github::repo`], the workflow from
/// `config.release_workflow`, and the entry names no environment.
///
/// # Errors
///
/// Returns an error, before any request to crates.io, when the owner's token
/// is unset. It also returns one when the default branch cannot be fetched,
/// the repository does not resolve, or `request.names` holds a crate the
/// repository does not upload.
pub fn run(config: &Config, root: &Path, request: &Request<'_>) -> Result<Vec<Row>> {
    let token = request.crates_io.owner_token.with_context(|| {
        format!(
            "release trust needs an owner's crates.io token with the trusted-publishing scope: set {OWNER}"
        )
    })?;
    let (crates, repository) = crates_from_env(config, root)?;
    let selected = select(&crates, request.names)?;
    let Some(repository) = repository else {
        return Ok(Vec::new());
    };
    let names: Vec<&str> = selected.iter().map(|listed| listed.name.as_str()).collect();
    Ok(add(
        &names,
        request.crates_io.api,
        token,
        &repository,
        &config.release_workflow,
        request.dry_run,
    ))
}

/// One row for each of `names`, each added to or left alone as [`run`]
/// describes.
pub(crate) fn add(
    names: &[&str],
    api: &str,
    token: &str,
    repository: &Repo,
    workflow: &str,
    dry_run: bool,
) -> Vec<Row> {
    names
        .iter()
        .map(|name| {
            let state =
                entry(name, api, token, repository, workflow, dry_run).unwrap_or_else(|error| {
                    State::Failed {
                        message: format!("{name}: {error:#}"),
                    }
                });
            Row::of(name, state)
        })
        .collect()
}

/// What happens to `name`'s entries.
fn entry(
    name: &str,
    api: &str,
    token: &str,
    repository: &Repo,
    workflow: &str,
    dry_run: bool,
) -> Result<State> {
    if registry::crates_io_user_owners(api, name)?.is_none() {
        return Ok(State::Failed {
            message: format!(
                "{name}: missing from crates.io; run `verctl release bootstrap {name}`"
            ),
        });
    }
    let entries = match registry::crates_io_github_configs(api, name, token)? {
        TrustedPublishers::Listed(entries) => entries,
        TrustedPublishers::Refused { detail } => {
            return Ok(State::Failed {
                message: format!(
                    "{name}: crates.io refused to list its trusted publishers ({detail}); the token's user must be an owner of the crate"
                ),
            });
        }
    };
    if entries
        .iter()
        .any(|entry| names(entry, repository, workflow))
    {
        return Ok(State::AlreadySet);
    }
    if entries.len() >= LIMIT {
        return Ok(State::Failed {
            message: format!(
                "{name}: holds {held} trusted publisher entries, the crates.io limit of {LIMIT}, and none names {repository} and {workflow}; remove one on crates.io",
                held = entries.len(),
                repository = slug(repository),
            ),
        });
    }
    if dry_run {
        return Ok(State::WouldAdd);
    }
    Ok(
        match registry::crates_io_add_github_config(api, name, token, repository, workflow)? {
            Added::Created => State::Added,
            Added::AtLimit { detail } => State::Failed {
                message: format!(
                    "{name}: at the crates.io limit of {LIMIT} trusted publisher entries ({detail})"
                ),
            },
            Added::Refused { detail } => State::Failed {
                message: format!("{name}: crates.io refused the entry ({detail})"),
            },
        },
    )
}
