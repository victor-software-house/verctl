//! Changesets-format fragments and changelog rendering.

#![allow(
    clippy::missing_errors_doc,
    reason = "every public function returns anyhow::Result whose context names the failure"
)]

pub mod assets;
pub mod bump;
pub mod changelog;
pub mod ci;
pub mod cli;
pub mod config;
pub mod detect;
pub mod driver;
pub mod fragment;
pub mod git;
pub mod github;
pub mod pins;
pub mod prepare;
pub mod process;
pub mod publish;
pub mod publisher;
pub mod release;
pub mod runners;
pub mod templates;
pub mod versions;

#[cfg(test)]
mod operator_docs;
