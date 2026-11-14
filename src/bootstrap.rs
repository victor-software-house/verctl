//! `release bootstrap`: `release --new`, then `release trust` for each crate
//! that run created.
//!
//! crates.io's trusted publishing cannot create a crate, and an entry can only
//! be added to a crate that exists. So the first version of a crate goes up
//! with the registry token in a release that may create crates
//! ([`NewCrates`]), and the trusted publisher entry follows for every crate
//! the release created ([`crate::trust`]).
//!
//! The release is the one `release` runs, with its proofs and its draft
//! Release. This module only chooses which crates it may create, and adds
//! the entries after it. A release the new-crate rate limit stops is run
//! again, as any release is, and the crates it created before the stop then
//! need `release trust`.

use std::path::Path;

use anyhow::{Result, ensure};

use crate::config::Config;
use crate::first_publish::{Row, State, select};
use crate::publish::{Allow, Outcome, PublishRun, Scope, run_pending};
use crate::registry::CratesIo;
use crate::registry_check::crates_from_env;
use crate::registry_token::{self, NewCrates, RegistryToken};
use crate::trust;

/// What `release bootstrap` runs with.
#[derive(Debug)]
pub struct Request<'a> {
    /// The crates the release may create, or none for every crate crates.io
    /// lacks.
    pub names: &'a [String],
    /// Whether to plan and prove only, and write nothing.
    pub dry_run: bool,
    /// What the release leaves out, declares and starts. Its `new_crates` is
    /// replaced by the crates `names` select.
    pub scope: Scope,
    /// The proofs the release may go past.
    pub allow: Allow,
    /// The registry token, and the trusted-publishing token when it is set.
    pub registry_token: Option<&'a RegistryToken>,
    /// The crates.io index and API, and the owner's token for the entries.
    pub crates_io: CratesIo<'a>,
}

/// Release with `--new`, then add the trusted publisher entry of each crate
/// the release created, and return both. Under `--dry-run` the second part
/// lists an entry it would add for each crate the release would create.
///
/// A crate named in `request.names` is the only kind the release may create,
/// and another crate crates.io lacks fails its proof.
///
/// # Errors
///
/// Returns an error, before any write, when a real run lacks the registry
/// token or the owner's token, naming each missing one. It also returns one
/// when `request.names` holds a crate the repository does not upload, and
/// under the conditions of [`run_pending`].
pub fn run(
    config: &Config,
    config_path: &Path,
    root: &Path,
    request: &Request<'_>,
) -> Result<(PublishRun, Vec<Row>)> {
    if !request.dry_run {
        require_tokens(request)?;
    }
    let (crates, repository) = crates_from_env(config, root)?;
    select(&crates, request.names)?;
    let scope = Scope {
        new_crates: if request.names.is_empty() {
            NewCrates::Every
        } else {
            NewCrates::Only(request.names.to_vec())
        },
        ..request.scope.clone()
    };
    let run = run_pending(
        config,
        config_path,
        root,
        request.dry_run,
        &scope,
        request.allow,
        request.registry_token,
        request.crates_io,
    )?;
    if run.created.is_empty() || run.outcome == Outcome::NothingToRelease {
        return Ok((run, Vec::new()));
    }
    let created: Vec<&str> = run.created.iter().map(String::as_str).collect();
    let rows = if request.dry_run {
        created
            .iter()
            .map(|name| Row::of(name, State::WouldAdd))
            .collect()
    } else if let (Some(repository), Some(token)) = (&repository, request.crates_io.owner_token) {
        trust::add(
            &created,
            request.crates_io.api,
            token,
            repository,
            &config.release_workflow,
            false,
        )
    } else {
        Vec::new()
    };
    Ok((run, rows))
}

/// Fail naming each token a real run lacks: the registry token that creates
/// the crates, and the owner's token that adds their entries, so no crate is
/// created without one.
fn require_tokens(request: &Request<'_>) -> Result<()> {
    let mut missing = Vec::new();
    if request.registry_token.is_none() {
        missing.push(format!(
            "{} is unset: set it to the crates.io token that creates new crates",
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
        "release bootstrap creates and registers nothing until each token is set: {}",
        missing.join("; ")
    );
    Ok(())
}
