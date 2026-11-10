use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{Arg, ArgMatches, Args, FromArgMatches, Parser, Subcommand};
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
    Version(VersionArgs),
    /// Publish the versions on HEAD (cargo / bun + GitHub Release).
    Publish(PublishArgs),
    /// Rewrite pins and render served templates at the versions on HEAD.
    Render(PublishArgs),
    /// Plan or build native GitHub Release tarballs declared under `assets`.
    Assets(AssetsArgs),
    /// Plan the validation jobs declared in `[ci]`.
    Ci(CiArgs),
    /// Wait for docs.rs to build each crates.io crate released at HEAD.
    Docs(DocsArgs),
    /// Check the API of each crates.io crate whose next release is a patch.
    Semver(SemverArgs),
    /// The hidden verbs `prepare` and `pin`, which fail and name their
    /// replacement.
    #[command(flatten)]
    Removed(Removed),
}

/// The verbs verctl 0.8.0 renamed, each paired with the command that
/// replaced it.
const REMOVED: [(&str, &str); 2] = [("prepare", "version"), ("pin", "render")];

/// A renamed verb. Each one is hidden from help, and the usage spec marks it
/// `hide`. It takes any arguments and fails the parse with a usage error, so
/// it exits 2, reads no config, and names the command that replaced it.
///
/// The enum has no variants: no parse ever produces one, so a handler has
/// nothing to run.
#[derive(Debug)]
pub enum Removed {}

impl Removed {
    /// The hidden command for `old`. It takes every argument, so the failure
    /// always names `new` and never an unexpected flag.
    fn command(old: &'static str, new: &'static str) -> clap::Command {
        clap::Command::new(old)
            .hide(true)
            .about(format!("Renamed to `{new}`."))
            .arg(
                Arg::new("ignored")
                    .help("Ignored: the command fails before it reads them.")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true)
                    .value_parser(clap::builder::OsStringValueParser::new())
                    .hide(true),
            )
    }
}

impl FromArgMatches for Removed {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let name = matches.subcommand_name().unwrap_or_default();
        Err(match REMOVED.iter().find(|(old, _)| *old == name) {
            Some((old, new)) => clap::Error::raw(
                ErrorKind::InvalidSubcommand,
                format!("{old} is now {new}: run verctl {new}"),
            ),
            None => clap::Error::new(ErrorKind::MissingSubcommand),
        })
    }

    fn update_from_arg_matches(&mut self, _: &ArgMatches) -> Result<(), clap::Error> {
        match *self {}
    }
}

impl Subcommand for Removed {
    fn augment_subcommands(command: clap::Command) -> clap::Command {
        command.subcommands(REMOVED.map(|(old, new)| Self::command(old, new)))
    }

    fn augment_subcommands_for_update(command: clap::Command) -> clap::Command {
        Self::augment_subcommands(command)
    }

    fn has_subcommand(name: &str) -> bool {
        REMOVED.iter().any(|(old, _)| *old == name)
    }
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

/// Arguments of `publish` and `render`.
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
    /// Plan, build, or upload this set under `vendored` instead of `assets`.
    #[arg(long, conflicts_with = "tag")]
    pub vendored: Option<String>,
    /// Build this target id (`darwin-arm64`, `linux-x64`). Plan only when
    /// omitted.
    #[arg(long)]
    pub build: Option<String>,
    /// Upload the built tarball to the GitHub Release for `--tag`. With
    /// `--vendored`, create the set's Release when it is missing and upload a
    /// `.sha256` beside the archive.
    #[arg(long)]
    pub upload: bool,
    /// Release tag (`v0.0.1`). Defaults to the first package's tag. A vendored
    /// set declares its own.
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
    /// Package map. The fragments are planned against it as `version` would.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Fail when a declared manifest version differs from the default branch.
    #[arg(long)]
    pub versions: bool,
}

/// Arguments of `version`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct VersionArgs {
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
}

impl VersionArgs {
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

    fn version_from(args: &[&str]) -> super::VersionArgs {
        let mut words = vec!["verctl", "version"];
        words.extend_from_slice(args);
        match parse(&words).expect("parse") {
            Cli {
                command: Command::Version(args),
                ..
            } => args,
            _ => panic!("expected version"),
        }
    }

    #[test]
    fn version_defaults_to_local() {
        let args = version_from(&[]);
        assert!(!args.open_pr());
        assert!(!args.no_pr);
        assert!(!args.pr);
    }

    #[test]
    fn version_no_pr_is_local() {
        let args = version_from(&["--no-pr"]);
        assert!(!args.open_pr());
        assert!(args.no_pr);
    }

    #[test]
    fn version_pr_is_settable() {
        let args = version_from(&["--pr"]);
        assert!(args.open_pr());
        assert!(args.pr);
        assert!(!args.no_pr);
    }

    #[test]
    fn last_pr_flag_wins() {
        assert!(!version_from(&["--pr", "--no-pr"]).open_pr());
        assert!(version_from(&["--no-pr", "--pr"]).open_pr());
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
        assert!(version_from(&["--preview"]).dry_run());
        assert!(version_from(&["--dry-run"]).dry_run());
        assert!(version_from(&["-n"]).dry_run());
    }

    #[test]
    fn format_json_works_before_and_after_the_verb() {
        for args in [
            ["verctl", "--format", "json", "render"].as_slice(),
            ["verctl", "render", "--format", "json"].as_slice(),
            ["verctl", "status", "--format", "json"].as_slice(),
            ["verctl", "-f", "json", "render"].as_slice(),
        ] {
            let cli = parse(args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
            assert!(cli.format.format.is_json(), "{args:?}");
        }
    }

    /// A renamed verb takes any arguments and fails the parse as a usage
    /// error that names its replacement.
    #[test]
    fn removed_verbs_name_their_replacement() {
        for (words, message) in [
            (
                ["verctl", "prepare", "--pr"].as_slice(),
                "prepare is now version: run verctl version",
            ),
            (
                ["verctl", "pin", "--dry-run"].as_slice(),
                "pin is now render: run verctl render",
            ),
            (
                ["verctl", "--format", "json", "pin"].as_slice(),
                "pin is now render: run verctl render",
            ),
        ] {
            let error = parse(words).expect_err("a removed verb must not parse");
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::InvalidSubcommand,
                "{words:?}"
            );
            assert_eq!(error.exit_code(), 2, "{words:?}");
            assert!(error.to_string().contains(message), "{words:?}: {error}");
        }
    }

    #[test]
    fn removed_verbs_are_hidden() {
        let command = Cli::command();
        for name in ["prepare", "pin"] {
            let removed = command
                .find_subcommand(name)
                .unwrap_or_else(|| panic!("{name}"));
            assert!(removed.is_hide_set(), "{name}");
        }
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
