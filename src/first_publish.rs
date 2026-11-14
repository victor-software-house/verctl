//! What `release trust` and the trust half of `release bootstrap` report: one
//! row per crate, and the crate selection both take.

use anyhow::{Result, bail};
use serde::Serialize;

use crate::registry_check::Crate;

/// One crate's trusted publisher entry.
///
/// Not `#[non_exhaustive]`: the binary builds one in its presentation tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// The crate name.
    pub name: String,
    /// What the step did, or would do under `--dry-run`.
    #[serde(flatten)]
    pub state: State,
}

/// What `release trust` did to a crate's entries.
///
/// Not `#[non_exhaustive]`: the binary labels every state, so a new one fails
/// its build until it has a label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum State {
    /// The trusted publisher entry was added.
    Added,
    /// An entry already names this repository and workflow.
    AlreadySet,
    /// `--dry-run`: the entry would be added.
    WouldAdd,
    /// The step failed. The run exits non-zero.
    Failed {
        /// What failed, naming the crate and what to change.
        message: String,
    },
}

impl State {
    /// The state in a few words, as the report's table shows it.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::AlreadySet => "already set",
            Self::WouldAdd => "would add",
            Self::Failed { .. } => "failed",
        }
    }
}

impl Row {
    /// A row for `name`.
    pub(crate) fn of(name: &str, state: State) -> Self {
        Self {
            name: name.to_owned(),
            state,
        }
    }

    /// Why the step failed, or `None` when it did not.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        match &self.state {
            State::Failed { message } => Some(message),
            State::Added | State::AlreadySet | State::WouldAdd => None,
        }
    }
}

/// The crates `names` select, in upload order: every crate when `names` is
/// empty.
///
/// # Errors
///
/// Returns an error naming each unknown crate and listing the crates the
/// repository uploads to crates.io.
pub(crate) fn select<'a>(crates: &'a [Crate], names: &[String]) -> Result<Vec<&'a Crate>> {
    let unknown: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| crates.iter().all(|listed| listed.name != *name))
        .collect();
    if !unknown.is_empty() {
        let known: Vec<&str> = crates.iter().map(|listed| listed.name.as_str()).collect();
        bail!(
            "no crate named {unknown} goes to crates.io from this repository; its crates are {known}",
            unknown = unknown.join(", "),
            known = if known.is_empty() {
                "none".to_owned()
            } else {
                known.join(", ")
            },
        );
    }
    Ok(crates
        .iter()
        .filter(|listed| names.is_empty() || names.contains(&listed.name))
        .collect())
}
