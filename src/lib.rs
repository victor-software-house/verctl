//! The library behind the `verctl` command: stack-agnostic version pull
//! requests from Changesets-format fragments.
//!
//! A repository declares its packages in `.ctl/ver.yaml` and writes one
//! Markdown fragment per change under `.changeset/`. verctl turns the pending
//! fragments into version bumps and changelog sections. It opens one Version
//! PR for them and publishes the versions once that pull request merges. The
//! command line in [`cli`] is the supported interface. The modules below are
//! the pieces it is built from. Every fallible function returns
//! [`anyhow::Result`] with context that names what failed.
//!
//! ## Quick start
//!
//! Parse a fragment and move a package's version by its bump:
//!
//! ```
//! use verctl::{bump, fragment};
//!
//! let raw = indoc::indoc! {"
//!     ---
//!     demo: minor
//!     ---
//!
//!     Read every flag from the environment.
//! "};
//! let fragment = fragment::parse_str(raw, ".changeset/env-flags.md")?;
//! let next = bump::apply("1.4.2", fragment.max_bump(), false)?;
//! assert_eq!(next, "1.5.0");
//! assert_eq!(fragment.summary, "Read every flag from the environment.");
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ## Release flow
//!
//! 1. [`config::Config`] is the parsed and validated `.ctl/ver.yaml`.
//! 2. [`fragment::load_dir`] reads the pending fragments, and [`prepare::plan`]
//!    turns them into one [`prepare::PlanEntry`] per package, with the next
//!    version from [`bump::apply_with`].
//! 3. [`prepare::apply_plan`] writes each version through its [`driver`],
//!    [`release::write_changelogs`] renders the changelog sections,
//!    [`prepare::run_steps`] runs each lockfile's owner and `prepare.after`,
//!    and [`release::open_or_update_pr`] opens or updates the Version PR.
//! 4. After the merge, [`assets`] builds the declared release tarballs, and
//!    [`publish`] uploads them to a draft GitHub Release, runs each package's
//!    [`publisher`], then publishes the draft, which tags the commit.
//! 5. [`versions`] guards the other way: a manifest version that differs from
//!    the default branch without a fragment fails the check. On the Version PR,
//!    [`content::check`] replays the version step on the merge-base and fails
//!    any change the replay does not make. On any other pull request,
//!    [`pull_request::problems`] lists each change only the Version PR may
//!    make.
//!
//! ## Cargo features
//!
//! verctl has no Cargo features. Every module is always built.

pub mod assets;
pub mod bootstrap;
/// The next version for a bump.
pub mod bump;
/// Changelog entries rendered from a release.
pub mod changelog;
pub mod ci;
/// The command line.
pub mod cli;
/// The `.ctl/ver.yaml` schema.
pub mod config;
pub mod content;
pub mod detect;
/// Reading and writing a version in a manifest.
pub mod driver;
/// What `release bootstrap` and `release trust` report.
pub mod first_publish;
/// Changeset fragments.
pub mod fragment;
pub mod freeze;
/// Git history the release reads.
pub mod git;
/// GitHub outputs, repositories, and pull requests.
pub mod github;
/// A Go module's version on its `go.mod` module line.
pub(crate) mod gomod;
/// The IMF-fixdate a registry gives for a retry.
pub(crate) mod http_date;
pub mod lockfile;
/// Lookups outside the commit, and the error for one that could not run.
pub mod lookup;
/// A package's version on one marker line of any file.
pub(crate) mod marker;
pub mod members;
/// Whether a manifest opts its package out of publishing.
pub(crate) mod opt_out;
pub mod pending;
pub mod pins;
/// The plan of version bumps for a release.
pub mod prepare;
/// Running external commands.
pub mod process;
pub mod publish;
pub mod publisher;
pub mod pull_request;
pub mod registry;
pub mod registry_check;
pub mod registry_token;
/// The token for `--pr` and the versions a release will carry.
pub mod release;
pub mod runners;
/// cargo-semver-checks at the release type the pending release allows.
pub mod semver_check;
pub mod stop;
pub mod templates;
pub mod trust;
pub mod versions;

#[cfg(test)]
mod operator_docs;
/// Fixture repositories built with the `git` binary.
#[cfg(test)]
pub(crate) mod test_git;
