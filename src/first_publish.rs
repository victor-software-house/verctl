//! What `release bootstrap` and `release trust` report: one row per crate and
//! step, and the crate selection both take.

use anyhow::{Result, bail};
use serde::Serialize;

use crate::registry_check::Planned;

/// One step of one crate.
///
/// Not `#[non_exhaustive]`: the binary builds one in its presentation tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// The crate name.
    pub name: String,
    /// The version `release bootstrap` publishes, or `None` for a row of
    /// `release trust`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// What the step did, or would do under `--dry-run`.
    #[serde(flatten)]
    pub state: State,
    /// What happened on the way, such as the wait for a rate limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What a step did to a crate.
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
    /// The crate was published.
    Published,
    /// `--dry-run`: the crate would be published.
    WouldPublish,
    /// crates.io already lists the crate.
    AlreadyPublished,
    /// The standalone package's tag exists at the default branch's tip.
    Tagged {
        /// The tag name.
        tag: String,
    },
    /// `--dry-run`: the standalone package would be tagged.
    WouldTag {
        /// The tag name.
        tag: String,
    },
    /// The step failed. The run exits non-zero.
    Failed {
        /// What failed, naming the crate and what to change.
        message: String,
    },
}

impl State {
    /// The state in a few words, as the report's table shows it.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Added => "added".to_owned(),
            Self::AlreadySet => "already set".to_owned(),
            Self::WouldAdd => "would add".to_owned(),
            Self::Published => "published".to_owned(),
            Self::WouldPublish => "would publish".to_owned(),
            Self::AlreadyPublished => "already published".to_owned(),
            Self::Tagged { tag } => format!("tagged {tag}"),
            Self::WouldTag { tag } => format!("would tag {tag}"),
            Self::Failed { .. } => "failed".to_owned(),
        }
    }
}

impl Row {
    /// A row with no version and no note.
    pub(crate) fn of(name: &str, state: State) -> Self {
        Self {
            name: name.to_owned(),
            version: None,
            state,
            note: None,
        }
    }

    /// A failed row for `name`, with `message`.
    pub(crate) fn failed(name: &str, message: String) -> Self {
        Self::of(name, State::Failed { message })
    }

    /// Why the step failed, or `None` when it did not.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        match &self.state {
            State::Failed { message } => Some(message),
            State::Added
            | State::AlreadySet
            | State::WouldAdd
            | State::Published
            | State::WouldPublish
            | State::AlreadyPublished
            | State::Tagged { .. }
            | State::WouldTag { .. } => None,
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
pub(crate) fn select<'a>(planned: &'a [Planned], names: &[String]) -> Result<Vec<&'a Planned>> {
    let unknown: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| planned.iter().all(|crate_| crate_.listed.name != *name))
        .collect();
    if !unknown.is_empty() {
        let known: Vec<&str> = planned
            .iter()
            .map(|crate_| crate_.listed.name.as_str())
            .collect();
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
    Ok(planned
        .iter()
        .filter(|crate_| names.is_empty() || names.contains(&crate_.listed.name))
        .collect())
}
