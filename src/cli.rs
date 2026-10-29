use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use ctl_core::{ColorLong, DryRunArgs, EnvPrefix, FormatArgs};

/// Stack-agnostic version PRs from Changesets-format fragments.
///
/// `env_prefix` binds every long option, at every depth, to `VERCTL_<LONG>`,
/// so an action or a CI step sets `env:` and runs the bare verb. A flag on the
/// command line wins over its variable, and an empty variable is unset.
#[derive(Debug, Parser)]
#[command(
    version,
    about = "Stack-agnostic version PRs from Changesets-format fragments",
    arg_required_else_help = true,
    env_prefix = "VERCTL"
)]
pub struct Cli {
    /// Output format selection shared by every command.
    #[command(flatten)]
    pub format: FormatArgs,
    /// Color selection shared by every command.
    #[command(flatten)]
    pub color: ColorLong,
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The verctl commands.
#[derive(Debug, Subcommand)]
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
    /// Plan, build, or upload the GitHub Release assets declared under
    /// `assets`.
    Assets(AssetsArgs),
    /// Plan the validation jobs declared in `[ci]`.
    Ci(CiArgs),
    /// Wait for docs.rs to build each crates.io crate released at HEAD.
    Docs(DocsArgs),
    /// Check the API of each crates.io crate whose next release is a patch.
    Semver(SemverArgs),
}

/// Arguments of `semver`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct SemverArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset", value_hint = clap::ValueHint::DirPath)]
    pub dir: PathBuf,
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
}

/// Arguments of `docs`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct DocsArgs {
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Minutes to wait for docs.rs before a crate it has not built fails, at
    /// most a day.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(0..=1440))]
    pub timeout: u64,
}

/// Arguments of `ci`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct CiArgs {
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Write `matrix` for `$GITHUB_OUTPUT`.
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub github_output: Option<PathBuf>,
}

/// Arguments of `publish` and `pin`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct PublishArgs {
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// The shared `--dry-run` flag.
    #[command(flatten)]
    pub dry: DryRunArgs,
    /// List every version commit whose release is incomplete, oldest first,
    /// and write `commits`, `pending`, and `assets` to `$GITHUB_OUTPUT` when
    /// it is set. Publishes nothing. `publish` only.
    #[arg(long)]
    pub plan: bool,
    /// Leave declared assets out of the completeness check, for a workflow
    /// that builds none. `publish` only.
    #[arg(long)]
    pub skip_assets: bool,
}

/// Arguments of `assets`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct AssetsArgs {
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Build this target id (`darwin-arm64`, `linux-x64`). Plan only when
    /// omitted.
    #[arg(long)]
    pub build: Option<String>,
    /// Upload the built tarball, or the `--file`, to the GitHub Release for
    /// `--tag`, draft or published.
    #[arg(long)]
    pub upload: bool,
    /// A file the repository built itself, declared under `assets.files`, to
    /// upload instead of building a target. Needs `--upload`.
    #[arg(long, requires = "upload", conflicts_with = "build", value_hint = clap::ValueHint::FilePath)]
    pub file: Option<PathBuf>,
    /// Release tag (`v0.0.1`). Defaults to the first package's tag.
    #[arg(long)]
    pub tag: Option<String>,
    /// Write `has_assets`, `tag`, and `matrix` for `$GITHUB_OUTPUT`.
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub github_output: Option<PathBuf>,
}

impl PublishArgs {
    /// Whether `--dry-run` was given.
    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry.dry_run
    }
}

/// Arguments of `status`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct StatusArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset", value_hint = clap::ValueHint::DirPath)]
    pub dir: PathBuf,
}

/// Arguments of `check`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct CheckArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset", value_hint = clap::ValueHint::DirPath)]
    pub dir: PathBuf,
    /// Package map. The fragments are planned against it as `prepare` would.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Fail when a declared manifest version differs from the default branch.
    #[arg(long)]
    pub versions: bool,
}

/// Arguments of `prepare`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct PrepareArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset", value_hint = clap::ValueHint::DirPath)]
    pub dir: PathBuf,
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// The shared `--dry-run` flag.
    #[command(flatten)]
    pub dry: DryRunArgs,
    /// Open or update the Version PR. Uses `GITHUB_TOKEN` / `GH_TOKEN` (not
    /// `gh`).
    #[arg(long, overrides_with = "no_pr")]
    pub pr: bool,
    /// Write files only. This is the default. Opposite of --pr.
    #[arg(long, overrides_with = "pr")]
    pub no_pr: bool,
    /// Enable GitHub auto-merge on the Version PR, as a squash titled after
    /// it. Needs `--pr`, a ruleset on the base that requires a status check,
    /// and a token other than the job's own (`VERCTL_JOB_TOKEN`).
    #[arg(long)]
    pub auto_merge: bool,
}

impl PrepareArgs {
    /// `true` when the operator asked to open a Version PR.
    #[must_use]
    pub fn open_pr(&self) -> bool {
        self.pr
    }

    /// Whether `--dry-run` was given.
    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry.dry_run
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::{CommandFactory, FromArgMatches};

    use super::{Cli, Command};

    /// Parse as `App` does: variables the command line overrules are unbound
    /// first, so an ambient `VERCTL_*` cannot change what a test asserts.
    fn parse(words: &[&str]) -> Result<Cli, clap::Error> {
        let raw: Vec<OsString> = words.iter().map(OsString::from).collect();
        let command = ctl_core::flags::env_under_flags(Cli::command(), &raw)?;
        Cli::from_arg_matches(&command.try_get_matches_from(raw)?)
    }

    fn prepare_from(args: &[&str]) -> super::PrepareArgs {
        let mut words = vec!["verctl", "prepare"];
        words.extend_from_slice(args);
        match parse(&words).expect("parse") {
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
    fn auto_merge_is_off_unless_asked() {
        assert!(!prepare_from(&["--pr"]).auto_merge);
        assert!(prepare_from(&["--pr", "--auto-merge"]).auto_merge);
    }

    #[test]
    fn check_versions_flag_parses() {
        let cli = parse(&["verctl", "check", "--versions"]).expect("parse");
        let Command::Check(args) = cli.command else {
            panic!("expected check");
        };
        assert!(args.versions);
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
            let cli = parse(args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
            assert!(cli.format.format.is_json(), "{args:?}");
        }
    }

    #[test]
    fn a_file_needs_upload_and_excludes_build() {
        assert!(parse(&["verctl", "assets", "--file", "build/App.pkg"]).is_err());
        assert!(
            parse(&[
                "verctl",
                "assets",
                "--upload",
                "--file",
                "build/App.pkg",
                "--build",
                "linux-x64",
            ])
            .is_err()
        );
        let cli =
            parse(&["verctl", "assets", "--upload", "--file", "build/App.pkg"]).expect("parse");
        let Command::Assets(args) = cli.command else {
            panic!("expected assets");
        };
        assert_eq!(
            args.file.as_deref(),
            Some(std::path::Path::new("build/App.pkg"))
        );
    }

    /// The release workflow sets these and runs the bare verb, so each must
    /// read its variable at subcommand depth.
    #[test]
    fn publish_flags_read_their_variables() {
        let command = Cli::command();
        let publish = command.find_subcommand("publish").expect("publish");
        for (id, variable) in [
            ("plan", "VERCTL_PLAN"),
            ("skip_assets", "VERCTL_SKIP_ASSETS"),
            ("config", "VERCTL_CONFIG"),
        ] {
            let arg = publish
                .get_arguments()
                .find(|arg| arg.get_id() == id)
                .unwrap_or_else(|| panic!("{id}"));
            assert_eq!(
                arg.get_env(),
                Some(OsString::from(variable).as_os_str()),
                "{id}"
            );
        }
    }
}
