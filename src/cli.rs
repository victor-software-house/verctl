use clap::{Args, Parser, Subcommand};
use ctl_core::{ColorLong, DryRunArgs, FormatArgs};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Stack-agnostic version PRs from Changesets-format fragments",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(flatten)]
    pub format: FormatArgs,
    #[command(flatten)]
    pub color: ColorLong,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Print the installed-version operator contract.
    Instructions,
    /// List pending .changeset fragments.
    Status(StatusArgs),
    /// Validate every fragment in a directory (fail closed).
    Check(StatusArgs),
    /// Apply fragment bumps. `--pr` opens the Version PR. `--dry-run` previews.
    Prepare(PrepareArgs),
    /// Publish the versions on HEAD (cargo / npm + GitHub Release).
    Publish(PublishArgs),
}

#[derive(Args)]
pub struct PublishArgs {
    /// Package map. Defaults to verctl.toml in the current directory.
    #[arg(short = 'c', long, default_value = "verctl.toml", value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    #[command(flatten)]
    pub dry: DryRunArgs,
}

impl PublishArgs {
    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry.dry_run
    }
}

#[derive(Args)]
pub struct StatusArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset", value_hint = clap::ValueHint::DirPath)]
    pub dir: PathBuf,
}

#[derive(Args)]
pub struct PrepareArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset", value_hint = clap::ValueHint::DirPath)]
    pub dir: PathBuf,
    /// Package map. Defaults to verctl.toml in the current directory.
    #[arg(short = 'c', long, default_value = "verctl.toml", value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    #[command(flatten)]
    pub dry: DryRunArgs,
    /// Open or update the Version PR. Uses `GITHUB_TOKEN` / `GH_TOKEN` (not `gh`).
    #[arg(long, overrides_with = "no_pr")]
    pub pr: bool,
    /// Write files only. This is the default. Opposite of --pr.
    #[arg(long, overrides_with = "pr")]
    pub no_pr: bool,
}

impl PrepareArgs {
    /// `true` when the operator asked to open a Version PR.
    #[must_use]
    pub fn open_pr(&self) -> bool {
        self.pr
    }

    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry.dry_run
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    fn prepare_from(args: &[&str]) -> super::PrepareArgs {
        let mut words = vec!["verctl", "prepare"];
        words.extend_from_slice(args);
        match Cli::try_parse_from(words).expect("parse") {
            Cli {
                command: Command::Prepare(args),
                ..
            } => args,
            _ => panic!("expected prepare"),
        }
    }

    #[test]
    fn prepare_defaults_to_local() {
        let args = prepare_from(&[]);
        assert!(!args.open_pr());
        assert!(!args.no_pr);
        assert!(!args.pr);
    }

    #[test]
    fn prepare_no_pr_is_local() {
        let args = prepare_from(&["--no-pr"]);
        assert!(!args.open_pr());
        assert!(args.no_pr);
    }

    #[test]
    fn prepare_pr_is_settable() {
        let args = prepare_from(&["--pr"]);
        assert!(args.open_pr());
        assert!(args.pr);
        assert!(!args.no_pr);
    }

    #[test]
    fn last_pr_flag_wins() {
        assert!(!prepare_from(&["--pr", "--no-pr"]).open_pr());
        assert!(prepare_from(&["--no-pr", "--pr"]).open_pr());
    }

    #[test]
    fn preview_is_dry_run() {
        assert!(prepare_from(&["--preview"]).dry_run());
        assert!(prepare_from(&["--dry-run"]).dry_run());
        assert!(prepare_from(&["-n"]).dry_run());
    }
}
