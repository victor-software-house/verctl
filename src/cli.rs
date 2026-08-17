use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Stack-agnostic version PRs from Changesets-format fragments",
    arg_required_else_help = true
)]
pub struct Cli {
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
    /// Apply fragment bumps to declared version files. Does not open a PR.
    Prepare(PrepareArgs),
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
    /// Print the plan and leave files alone.
    #[arg(long)]
    pub dry_run: bool,
    /// Local only. The Version PR lands in a later slice.
    #[arg(long, default_value_t = true)]
    pub no_pr: bool,
}
