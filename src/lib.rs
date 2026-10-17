//! Changesets-format fragments and changelog rendering.

#![allow(
    clippy::missing_errors_doc,
    reason = "every public function returns anyhow::Result whose context names the failure"
)]

pub mod assets;
/// The next version for a bump.
pub mod bump;
/// Changelog entries rendered from a release.
pub mod changelog;
pub mod ci;
/// The command line.
pub mod cli;
/// The `.ctl/ver.yaml` schema.
pub mod config;
pub mod detect;
/// Reading and writing a version in a manifest.
pub mod driver;
/// Changeset fragments.
pub mod fragment;
/// Git history the release reads.
pub mod git;
/// GitHub outputs, repositories, and pull requests.
pub mod github;
pub mod pending;
pub mod pins;
/// The plan of version bumps for a release.
pub mod prepare;
/// Running external commands.
pub mod process;
pub mod publish;
pub mod publisher;
pub mod registry;
/// The token for `--pr` and the versions a release will carry.
pub mod release;
pub mod runners;
pub mod templates;
pub mod versions;

#[cfg(test)]
mod operator_docs;
/// Fixture repositories built with the `git` binary.
#[cfg(test)]
pub(crate) mod test_git;
