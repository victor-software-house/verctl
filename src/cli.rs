use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
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
    /// List pending fragments, the asset plan, and HEAD's incomplete
    /// releases. `--github-output` writes the release workflow's plan.
    Status(StatusArgs),
    /// Run the named checks, or `fragments` and `served` when none is named.
    Check(CheckArgs),
    /// Apply fragment bumps. `--pr` opens the Version PR. `--dry-run` previews.
    Version(VersionArgs),
    /// Release the versions on HEAD: publish with cargo or bun, and create the
    /// GitHub Release.
    /// `release asset <target>` builds one asset.
    Release(ReleaseArgs),
    /// Rewrite pins and render served templates at the versions on HEAD.
    Render(RenderArgs),
    /// Plan the validation jobs declared in `[ci]`.
    Ci(CiArgs),
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

/// Arguments of `release`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct ReleaseArgs {
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// The shared `--dry-run` flag.
    #[command(flatten)]
    pub dry: DryRunArgs,
    /// Leave declared assets out of the completeness check, for a workflow
    /// that builds none.
    #[arg(long)]
    pub skip_assets: bool,
    /// The file names of the assets the release workflow's plan expects at
    /// this version commit, comma separated. `release` then declares exactly
    /// these, so a leg declares what the plan scheduled. Without it,
    /// `release` declares the targets this commit's own config names.
    #[arg(long, value_delimiter = ',')]
    pub declared_assets: Vec<String>,
    /// Create and publish tags and Releases only. `release` runs no publisher,
    /// and refuses a package whose version the registry does not list, so a
    /// job that holds no registry credential cannot upload.
    #[arg(long)]
    pub releases_only: bool,
    /// The directory holding the tarballs `release asset` built. `release`
    /// uploads each declared one its draft, or its mutable published Release,
    /// lacks before any registry upload, and fails before it writes anything
    /// when one is missing. Without it, the Release must already hold every
    /// declared asset.
    #[arg(long, value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    pub assets_dir: Option<PathBuf>,
    /// The proofs `release` may go past.
    #[command(flatten)]
    pub allow: AllowArgs,
    /// The variable each publisher reads the registry token from, such as
    /// `CARGO_REGISTRY_TOKEN` for crates.io or `CARGO_REGISTRIES_<NAME>_TOKEN`
    /// for another cargo registry. The token itself comes from
    /// `VERCTL_REGISTRY_TOKEN` or `VERCTL_TRUSTED_PUBLISHING_TOKEN`, and no
    /// flag takes it, so it never reaches an argv. Only the publisher and its
    /// registry `query` run with it.
    #[arg(long, default_value = "CARGO_REGISTRY_TOKEN", value_parser = variable_name)]
    pub registry_token_env: String,
    /// The crates.io API root that `release` reads owners and trusted
    /// publishers from. Point it at crates.io's staging instance,
    /// `https://staging.crates.io/api/v1`, to prove a staging release.
    #[arg(long, value_name = "URL", default_value = crate::registry::CRATES_IO_API)]
    pub crates_io_api: String,
    /// The crates.io sparse index root that `release` reads to learn whether a
    /// crate version is already published. Point it at crates.io's staging
    /// index, `https://index.staging.crates.io`, for a staging release.
    #[arg(long, value_name = "URL", default_value = crate::registry::CRATES_IO_INDEX)]
    pub crates_io_index: String,
    /// `release asset <target>`. Without it, `release` publishes.
    #[command(subcommand)]
    pub step: Option<ReleaseStep>,
}

/// The value parser of `--registry-token-env`: a variable name as POSIX
/// defines one, letters, digits, and underscores that do not start with a
/// digit ([POSIX Name]). A name outside that set could not be exported by a
/// shell, and a publisher run from one would never see the token.
///
/// [POSIX Name]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_216
fn variable_name(value: &str) -> Result<String, String> {
    let mut chars = value.chars();
    let starts = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic());
    if starts && chars.all(|rest| rest == '_' || rest.is_ascii_alphanumeric()) {
        Ok(value.to_owned())
    } else {
        Err(format!(
            "{value:?} is not a variable name: use letters, digits, and underscores, and start with a letter or an underscore"
        ))
    }
}

/// The steps under `release`.
///
/// Not `#[non_exhaustive]`: the binary is its own crate and matches the steps
/// exhaustively, as it does the command list.
/// A new step then fails its build until it is run.
#[derive(Debug, Subcommand)]
pub enum ReleaseStep {
    /// Build one native tarball that `assets` declares for the GitHub Release.
    /// `release --assets-dir` uploads it. A vendored set's archive is built
    /// and uploaded here, unless `--dry-run`.
    Asset(AssetArgs),
}

/// The proofs `release` may go past, each an independent switch with its own
/// variable.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct AllowArgs {
    /// Release even when `.github/workflows/` at HEAD differs from the
    /// default branch's tip. The report lists it under `allowed`.
    #[arg(long)]
    pub allow_workflow_change: bool,
    /// Release from a commit the default branch does not contain. The report
    /// lists it under `allowed`.
    #[arg(long)]
    pub allow_off_default_branch: bool,
}

/// Arguments of `release asset`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct AssetArgs {
    /// The target id to build (`darwin-arm64`, `linux-x64`). A workflow sets
    /// `VERCTL_TARGET` and runs the bare verb.
    #[arg(env = "VERCTL_TARGET")]
    pub target: String,
    /// Package map. Defaults to the one named before `asset`, which defaults
    /// to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, value_hint = clap::ValueHint::FilePath)]
    pub config: Option<PathBuf>,
    /// The shared `--dry-run` flag: build and upload nothing for a vendored
    /// set. A package tarball is built either way and never uploaded here.
    #[command(flatten)]
    pub dry: DryRunArgs,
    /// Build and upload this set under `vendored` instead of `assets`. With
    /// it, the upload creates the set's Release when it is missing and adds a
    /// `.sha256` beside the archive.
    #[arg(long)]
    pub vendored: Option<String>,
    /// The already-built release binary for this target. With it, `release
    /// asset` runs neither `prepare` nor `build` and packs this file as the
    /// binary. A target that packs `files` refuses it. A workflow sets
    /// `VERCTL_PREBUILT`; an empty variable is unset.
    #[arg(long, value_name = "PATH", value_hint = clap::ValueHint::FilePath, conflicts_with = "vendored")]
    pub prebuilt: Option<PathBuf>,
}

impl ReleaseArgs {
    /// Whether `--dry-run` was given.
    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry.dry_run
    }
}

impl AssetArgs {
    /// Whether `--dry-run` was given.
    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry.dry_run
    }

    /// Whether `--dry-run` was given before or after `asset`, so a flag written
    /// at the `release` level can never turn a dry run into an upload.
    #[must_use]
    pub fn dry_run_under(&self, release: &ReleaseArgs) -> bool {
        self.dry_run() || release.dry_run()
    }

    /// The config named after `asset`, or else the one named before it.
    #[must_use]
    pub fn config_under<'a>(&'a self, release: &'a ReleaseArgs) -> &'a Path {
        self.config.as_deref().unwrap_or(&release.config)
    }
}

/// Arguments of `render`.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct RenderArgs {
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// The shared `--dry-run` flag.
    #[command(flatten)]
    pub dry: DryRunArgs,
}

impl RenderArgs {
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
    /// Package map. Defaults to .ctl/ver.yaml in the current directory.
    #[arg(short = 'c', long, default_value = crate::config::FILE, value_hint = clap::ValueHint::FilePath)]
    pub config: PathBuf,
    /// Plan this set under `vendored` instead of the packages' `assets`.
    #[arg(long)]
    pub vendored: Option<String>,
    /// Leave declared assets out of the completeness check, for a workflow
    /// that builds none.
    #[arg(long)]
    pub skip_assets: bool,
    /// Fetch the full history and write the release workflow's plan to this
    /// file: `has_assets`, `tag`, `matrix`, `commits`, `pending`, and
    /// `assets`. Needs `GITHUB_TOKEN` or `GH_TOKEN`. With `--vendored` it
    /// writes only `has_assets`, `tag`, and `matrix`, and needs no token.
    #[arg(long, value_hint = clap::ValueHint::FilePath)]
    pub github_output: Option<PathBuf>,
    /// The crates.io sparse index root that `status` reads to learn whether a
    /// crate version is already published. Point it at crates.io's staging
    /// index, `https://index.staging.crates.io`, for a staging release.
    #[arg(long, value_name = "URL", default_value = crate::registry::CRATES_IO_INDEX)]
    pub crates_io_index: String,
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
    /// The checks to run. With none, `check` runs `fragments` and `served`,
    /// which need only the checkout.
    #[arg(value_name = "CHECK")]
    pub checks: Vec<Check>,
    /// Run every check. Any check named beside it runs once, with the rest.
    #[arg(short = 'a', long)]
    pub all: bool,
    /// The crates.io API root that `registry` and `trust` reads owners and
    /// trusted publishers from. Point it at crates.io's staging instance,
    /// `https://staging.crates.io/api/v1`, to prove a staging release.
    #[arg(long, value_name = "URL", default_value = crate::registry::CRATES_IO_API)]
    pub crates_io_api: String,
}

impl CheckArgs {
    /// The checks this run makes, each once, in the order [`Check`] declares
    /// them, whatever order the command line names them in.
    #[must_use]
    pub fn selected(&self) -> Vec<Check> {
        let named: &[Check] = if self.checks.is_empty() {
            &Check::DEFAULT
        } else {
            &self.checks
        };
        Check::value_variants()
            .iter()
            .copied()
            .filter(|check| self.all || named.contains(check))
            .collect()
    }
}

/// A check `check` runs by name, as `cargo deny check` runs its named checks.
/// Names add up and never overlap.
///
/// Not `#[non_exhaustive]`: the binary matches every check, so a new one
/// fails its build until it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Check {
    /// Every pending fragment parses, and `version` would accept it.
    Fragments,
    /// Every served file and pin matches what `render` writes.
    Served,
    /// Each declared manifest version matches the default branch, outside the
    /// Version PR.
    Versions,
    /// Each crates.io crate the default branch holds exists and lists every
    /// login `owners` names; reports whether its newest version came through
    /// trusted publishing. Needs the network and no token.
    Registry,
    /// Each crates.io crate has a trusted publisher naming this repository and
    /// `release_workflow`. Needs an owner's token in `VERCTL_OWNER_TOKEN`.
    Trust,
}

impl Check {
    /// The checks a bare `check` runs: both need only the checkout, with no
    /// network and no token.
    pub const DEFAULT: [Self; 2] = [Self::Fragments, Self::Served];
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
    use std::path::Path;

    use clap::error::ErrorKind;
    use clap::{CommandFactory, FromArgMatches};
    use ctl_core::flags;

    use super::{Check, Cli, Command};

    /// Parse as `App` does: variables the command line overrules are unbound
    /// first, so an ambient `VERCTL_*` cannot change what a test asserts.
    fn parse(words: &[&str]) -> Result<Cli, clap::Error> {
        let raw: Vec<OsString> = words.iter().map(OsString::from).collect();
        let command = flags::env_under_flags(Cli::command(), &raw)?;
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

    fn checks_from(args: &[&str]) -> Vec<Check> {
        let mut words = vec!["verctl", "check"];
        words.extend_from_slice(args);
        match parse(&words).expect("parse") {
            Cli {
                command: Command::Check(args),
                ..
            } => args.selected(),
            _ => panic!("expected check"),
        }
    }

    /// Bare `check` runs the offline checks. Names run once each, in the
    /// order `Check` declares them, and `--all` runs every check.
    #[test]
    fn check_runs_the_checks_it_names() {
        assert_eq!(checks_from(&[]), [Check::Fragments, Check::Served]);
        assert_eq!(
            checks_from(&["versions", "served"]),
            [Check::Served, Check::Versions]
        );
        assert_eq!(checks_from(&["versions", "versions"]), [Check::Versions]);
        assert_eq!(
            checks_from(&["trust", "registry"]),
            [Check::Registry, Check::Trust]
        );
        let every = [
            Check::Fragments,
            Check::Served,
            Check::Versions,
            Check::Registry,
            Check::Trust,
        ];
        assert_eq!(checks_from(&["--all"]), every);
        assert_eq!(checks_from(&["-a"]), every);
        assert_eq!(checks_from(&["-a", "versions"]), every);
    }

    /// An unknown name fails the parse and lists the known ones.
    #[test]
    fn an_unknown_check_lists_the_known_names() {
        let error = parse(&["verctl", "check", "release"]).expect_err("release is no check");
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("[possible values: fragments, served, versions, registry, trust]"),
            "{error}"
        );
    }

    /// `check --versions` is gone, with no hidden flag: `check versions`
    /// replaced it.
    #[test]
    fn check_takes_no_versions_flag() {
        let error = parse(&["verctl", "check", "--versions"]).expect_err("--versions is gone");
        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
        assert_eq!(error.exit_code(), 2);
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

    /// The verbs 0.8.0 renamed, and `release --plan`, are unknown: verctl
    /// keeps no hidden command or flag for an old name.
    #[test]
    fn the_renamed_verbs_and_release_plan_are_unknown() {
        for (words, kind) in [
            (
                ["verctl", "publish", "--dry-run"].as_slice(),
                ErrorKind::InvalidSubcommand,
            ),
            (
                ["verctl", "assets", "--build", "linux-x64"].as_slice(),
                ErrorKind::InvalidSubcommand,
            ),
            (
                ["verctl", "prepare", "--pr"].as_slice(),
                ErrorKind::InvalidSubcommand,
            ),
            (
                ["verctl", "pin", "--dry-run"].as_slice(),
                ErrorKind::InvalidSubcommand,
            ),
            (
                ["verctl", "release", "--plan"].as_slice(),
                ErrorKind::UnknownArgument,
            ),
        ] {
            let error = parse(words).expect_err("an old name must not parse");
            assert_eq!(error.kind(), kind, "{words:?}");
            assert_eq!(error.exit_code(), 2, "{words:?}");
        }
        let command = Cli::command();
        for name in ["prepare", "pin", "publish", "assets"] {
            assert!(command.find_subcommand(name).is_none(), "{name}");
        }
        assert!(parse(&["verctl", "release", "--dry-run"]).is_ok());
    }

    /// `--registry-token-env` takes a variable name a shell could export, and
    /// anything else fails the parse as a usage error before any work.
    #[test]
    fn registry_token_env_takes_a_variable_name() {
        for name in ["CARGO_REGISTRY_TOKEN", "NODE_AUTH_TOKEN", "_token2"] {
            let cli = parse(&["verctl", "release", "--registry-token-env", name]).expect(name);
            let Command::Release(args) = cli.command else {
                panic!("expected release");
            };
            assert_eq!(args.registry_token_env, name);
        }
        for name in ["", "1TOKEN", "CARGO-TOKEN", "TOKEN=x", "A B"] {
            let error = parse(&["verctl", "release", "--registry-token-env", name])
                .expect_err("a bad name must not parse");
            assert_eq!(error.exit_code(), 2, "{name:?}");
            assert!(
                error.to_string().contains("is not a variable name"),
                "{name:?}: {error}"
            );
        }
    }

    /// `release asset` takes its target as an argument or from
    /// `VERCTL_TARGET`, so a workflow sets `env:` and runs the bare verb.
    #[test]
    fn release_asset_takes_its_target() {
        let cli = parse(&["verctl", "release", "asset", "linux-x64", "--dry-run"]).expect("parse");
        let Command::Release(super::ReleaseArgs {
            step: Some(super::ReleaseStep::Asset(asset)),
            ..
        }) = cli.command
        else {
            panic!("expected release asset");
        };
        assert_eq!(asset.target, "linux-x64");
        assert!(asset.dry_run());
        let command = Cli::command();
        let asset = command
            .find_subcommand("release")
            .and_then(|release| release.find_subcommand("asset"))
            .expect("release asset");
        let target = asset
            .get_arguments()
            .find(|arg| arg.get_id() == "target")
            .expect("target");
        assert_eq!(
            target.get_env(),
            Some(OsString::from("VERCTL_TARGET").as_os_str())
        );
    }

    /// `release asset --prebuilt` names the binary to pack, reads
    /// `VERCTL_PREBUILT`, and refuses to combine with `--vendored`, whose
    /// archive packs more than one binary.
    #[test]
    fn release_asset_takes_a_prebuilt_binary() {
        let cli = parse(&[
            "verctl",
            "release",
            "asset",
            "linux-x64",
            "--prebuilt",
            "build/verctl",
        ])
        .expect("parse");
        let Command::Release(super::ReleaseArgs {
            step: Some(super::ReleaseStep::Asset(asset)),
            ..
        }) = cli.command
        else {
            panic!("expected release asset");
        };
        assert_eq!(asset.prebuilt.as_deref(), Some(Path::new("build/verctl")));
        let command = Cli::command();
        let prebuilt = command
            .find_subcommand("release")
            .and_then(|release| release.find_subcommand("asset"))
            .and_then(|asset| asset.get_arguments().find(|arg| arg.get_id() == "prebuilt"))
            .expect("prebuilt");
        assert_eq!(
            prebuilt.get_env(),
            Some(OsString::from("VERCTL_PREBUILT").as_os_str())
        );
        let error = parse(&[
            "verctl",
            "release",
            "asset",
            "linux-x64",
            "--prebuilt",
            "build/verctl",
            "--vendored",
            "libexample",
        ])
        .expect_err("--prebuilt and --vendored must not combine");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    /// The release workflow sets these and runs the bare verb, so each must
    /// read its variable at subcommand depth.
    #[test]
    fn release_flags_read_their_variables() {
        let command = Cli::command();
        let release = command.find_subcommand("release").expect("release");
        for (id, variable) in [
            ("skip_assets", "VERCTL_SKIP_ASSETS"),
            ("declared_assets", "VERCTL_DECLARED_ASSETS"),
            ("releases_only", "VERCTL_RELEASES_ONLY"),
            ("allow_workflow_change", "VERCTL_ALLOW_WORKFLOW_CHANGE"),
            (
                "allow_off_default_branch",
                "VERCTL_ALLOW_OFF_DEFAULT_BRANCH",
            ),
            ("config", "VERCTL_CONFIG"),
            ("registry_token_env", "VERCTL_REGISTRY_TOKEN_ENV"),
            ("crates_io_api", "VERCTL_CRATES_IO_API"),
            ("crates_io_index", "VERCTL_CRATES_IO_INDEX"),
        ] {
            let arg = release
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
