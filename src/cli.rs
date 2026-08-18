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
    /// Validate fragments, or `--versions` against the default branch.
    Check(CheckArgs),
    /// Apply fragment bumps. `--pr` opens the Version PR. `--dry-run` previews.
    Prepare(PrepareArgs),
    /// Publish the versions on HEAD (cargo / bun + GitHub Release).
    Publish(PublishArgs),
    /// Rewrite collocated tool pins to the versions on HEAD.
    Pin(PublishArgs),
    /// Plan or build native GitHub Release tarballs declared in `[assets]`.
    Assets(AssetsArgs),
    /// Plan the validation jobs declared in `[ci]`.
    Ci(CiArgs),
}

#[derive(Args)]
pub struct CiArgs {
    /// Package map. Defaults to verctl.toml in the current directory.
    #[arg(short = 'c', long, default_value = "verctl.toml", value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Write `matrix` for `$GITHUB_OUTPUT`.
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub github_output: Option<PathBuf>,
}

#[derive(Args)]
pub struct PublishArgs {
    /// Package map. Defaults to verctl.toml in the current directory.
    #[arg(short = 'c', long, default_value = "verctl.toml", value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    #[command(flatten)]
    pub dry: DryRunArgs,
}

#[derive(Args)]
pub struct AssetsArgs {
    /// Package map. Defaults to verctl.toml in the current directory.
    #[arg(short = 'c', long, default_value = "verctl.toml", value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Build this target id (`darwin-arm64`, `linux-x64`). Plan only when omitted.
    #[arg(long)]
    pub build: Option<String>,
    /// Upload the built tarball to the GitHub Release for `--tag`.
    #[arg(long)]
    pub upload: bool,
    /// Release tag (`v0.0.1`). Required with `--upload`.
    #[arg(long)]
    pub tag: Option<String>,
    /// Write `has_assets`, `tag`, and `matrix` for `$GITHUB_OUTPUT`.
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub github_output: Option<PathBuf>,
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
pub struct CheckArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset", value_hint = clap::ValueHint::DirPath)]
    pub dir: PathBuf,
    /// Package map. Used with `--versions`.
    #[arg(short = 'c', long, default_value = "verctl.toml", value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Fail when a declared manifest version differs from the default branch.
    #[arg(long)]
    pub versions: bool,
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
    fn check_versions_flag_parses() {
        let cli = Cli::try_parse_from(["verctl", "check", "--versions"]).expect("parse");
        match cli.command {
            Command::Check(args) => assert!(args.versions),
            _ => panic!("expected check"),
        }
    }

    #[test]
    fn preview_is_dry_run() {
        assert!(prepare_from(&["--preview"]).dry_run());
        assert!(prepare_from(&["--dry-run"]).dry_run());
        assert!(prepare_from(&["-n"]).dry_run());
    }

    #[test]
    fn format_json_works_before_and_after_the_verb() {
        for args in [
            ["verctl", "--format", "json", "pin"].as_slice(),
            ["verctl", "pin", "--format", "json"].as_slice(),
            ["verctl", "status", "--format", "json"].as_slice(),
            ["verctl", "-f", "json", "pin"].as_slice(),
        ] {
            let cli = Cli::try_parse_from(args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
            assert!(cli.format.format.is_json(), "{args:?}");
        }
    }
}
