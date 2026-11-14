//! The `verctl` binary: parses the command line and runs the chosen command.

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use ctl_core::prelude::*;
use verctl::cli::{
    AfterVerb, AssetArgs, BootstrapArgs, Check, CheckArgs, CiArgs, Cli, Command, ReleaseArgs,
    ReleaseOptions, ReleaseStep, RenderArgs, SemverArgs, StatusArgs, TrustArgs, VersionArgs,
};
use verctl::config::{self, Config};
use verctl::content::Inputs;
use verctl::fragment::{self, Bump};
use verctl::registry_token::{NewCrates, RegistryToken};
use verctl::{
    assets, bootstrap, ci, first_publish, freeze, git, github, lookup, pending, pins, prepare,
    publish, registry, registry_check, registry_token, release, semver_check, templates, trust,
    versions,
};

mod presentation;

use presentation::{
    AssetsReport, CheckReport, CheckResult, CiReport, CratesReport, InstructionsReport, PartialRow,
    ReleaseReport, ReleaseRow, RenderReport, Report, SemverReport, StatusAssets, StatusFragment,
    StatusPackage, StatusReport, Unchecked, VendoredReport, VersionBump, VersionCheckReport,
    VersionReport,
};

const INSTRUCTIONS: &str = include_str!("instructions.md");

fn main() -> ExitCode {
    App::<Cli>::new("verctl")
        .mounted_as("ver")
        .view(|cli| cli.format.view(cli.color.color()))
        .run(execute)
}

fn execute(cli: Cli) -> Result<Report> {
    match cli.command {
        Command::Instructions => Ok(Report::Instructions(InstructionsReport::new(INSTRUCTIONS))),
        Command::Status(args) => status_report(&args).map(Report::Status),
        Command::Check(args) => check_report(&args).map(Report::Check),
        Command::Version(args) => version_report(&args).map(Report::Version),
        Command::Release(args) => match &args.step {
            Some(ReleaseStep::Asset(asset)) => match &asset.vendored {
                Some(name) => vendored_report(asset, &args, name).map(Report::Vendored),
                None => asset_report(asset, &args).map(Report::Assets),
            },
            Some(ReleaseStep::Bootstrap(step)) => {
                bootstrap_report(step, &args).map(Report::Release)
            }
            Some(ReleaseStep::Trust(step)) => trust_report(step, &args).map(Report::Crates),
            None => release_report(&args).map(Report::Release),
        },
        Command::Render(args) => render_report(&args).map(Report::Render),
        Command::Ci(args) => ci_report(&args).map(Report::Ci),
        Command::Semver(args) => semver_report(&args).map(Report::Semver),
    }
}

fn semver_report(args: &SemverArgs) -> Result<SemverReport> {
    let config = Config::load(&args.config)?;
    let fragments = fragment::load_dir(&args.dir)?;
    Ok(SemverReport {
        crates: semver_check::run(&config, &config::root_of(&args.config), &fragments)?,
    })
}

/// Each check [`CheckArgs::selected`] names, in order. A check that cannot
/// finish fails the run with its own error, and the checks after it do not
/// run.
fn check_report(args: &CheckArgs) -> Result<CheckReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let checks = args
        .selected()
        .into_iter()
        .map(|check| match check {
            Check::Fragments => check_fragments(args, &config, root),
            Check::Served => check_served(root, &config).map(|()| CheckResult::Served),
            Check::Versions => check_versions(args, &config, root).map(CheckResult::Versions),
            Check::Registry => check_registry(args, &config, root).map(CheckResult::Registry),
            Check::Trust => registry_check::trust(
                &config,
                root,
                &args.crates_io_api,
                registry_token::owner_token().as_deref(),
            )
            .map(CheckResult::Trust),
        })
        .collect::<Result<_>>()?;
    Ok(CheckReport { checks })
}

/// The `registry` proofs and, after them, the release freeze. The freeze
/// follows the proofs because they fetch the default branch it reads.
fn check_registry(
    args: &CheckArgs,
    config: &Config,
    root: &Path,
) -> Result<registry_check::RegistryReport> {
    let report = registry_check::registry(config, root, &args.crates_io_api)?;
    let settings = freeze::Settings::new(
        &args.crates_io_index,
        Duration::from_secs(args.release_wait),
    );
    let freeze = freeze::check(config, &args.config, root, &settings)?;
    Ok(registry_check::RegistryReport {
        freeze: Some(freeze),
        ..report
    })
}

/// Fragments that parse and that `version` would accept. Every package they
/// name is declared, and no bump in them is refused.
fn check_fragments(args: &CheckArgs, config: &Config, root: &Path) -> Result<CheckResult> {
    let fragments = fragment::load_dir(&args.dir)?;
    if !fragments.is_empty() {
        prepare::plan(config, &fragments, root)?;
    }
    Ok(CheckResult::Fragments {
        ok: fragments.len(),
    })
}

/// Every template target and `pins` file that differs from what verctl would
/// write at the versions the manifests hold. One error lists them all, so one
/// run names every file to fix. It runs whether or not a fragment is pending,
/// so a hand edit fails here before the next release overwrites it.
fn check_served(root: &Path, config: &Config) -> Result<()> {
    let mut problems = Vec::new();
    if !config.pins.is_empty() {
        let versions = pins::current_versions(root, config)?;
        problems.extend(
            pins::stale(root, &config.pins, &versions)
                .into_iter()
                .map(|pin| format!("{}: {}", pin.file.display(), pin.problem)),
        );
    }
    if templates::any(root, &config.templates)? {
        let versions = release::served_versions(root, config, &[]);
        problems.extend(
            templates::drift(root, &config.templates, &config.pins, &versions)?
                .into_iter()
                .map(|drift| {
                    format!(
                        "{}: differs from what {} renders",
                        drift.target.display(),
                        drift.template.display()
                    )
                }),
        );
    }
    if problems.is_empty() {
        return Ok(());
    }
    bail!(
        "{count} served file(s) differ from the versions on HEAD; run `verctl render` to write them:\n{list}",
        count = problems.len(),
        list = problems.join("\n"),
    );
}

/// Each declared manifest version against the merge-base of HEAD and the
/// default branch.
fn check_versions(args: &CheckArgs, config: &Config, root: &Path) -> Result<VersionCheckReport> {
    let inputs = Inputs::new(&args.config, &args.dir);
    let report = versions::report(root, config, &inputs)?;
    Ok(VersionCheckReport {
        skip: report.skip,
        rows: report.rows,
        notices: report.notices,
    })
}

/// The versions templates render from.
///
/// A pin moves only what this release names. A template renders a whole file,
/// so a served file must also show the current version of each package that
/// no fragment bumped. That map costs a read of every manifest, so a repo that
/// serves nothing from a template never builds it.
fn versions_for_templates(
    root: &Path,
    config: &Config,
    planned: &[(String, String)],
) -> Result<Vec<(String, String)>> {
    if templates::any(root, &config.templates)? {
        return Ok(release::served_versions(root, config, planned));
    }
    Ok(planned.to_vec())
}

/// The token and Version PR that `version --pr` writes with, settled before
/// anything is written; `None` when the run opens no pull request.
fn version_pr_target(
    args: &VersionArgs,
    config: &Config,
    root: &Path,
) -> Result<Option<(String, release::VersionPr)>> {
    if !args.open_pr() || args.dry_run() {
        return Ok(None);
    }
    let token = release::resolve_token()?;
    let version_pr = release::VersionPr::new(&config.proposal, root)?;
    Ok(Some((token, version_pr)))
}

fn version_report(args: &VersionArgs) -> Result<VersionReport> {
    let dry_run = args.dry_run();
    let config = Config::load(&args.config)?;
    let fragments = fragment::load_dir(&args.dir)?;
    let root = &config::root_of(&args.config);
    let plan = prepare::plan(&config, &fragments, root)?;
    if plan.is_empty() && release::contributing_fragments(&plan, &fragments).is_empty() {
        return Ok(VersionReport {
            bumps: Vec::new(),
            changelog: String::new(),
            consume: Vec::new(),
            pins: Vec::new(),
            pr: args.open_pr().then(|| "no-op".to_owned()),
            after: Vec::new(),
            dry_run,
        });
    }
    let recovered = release::recovered_fragments(root, &args.dir, &plan, &fragments)?;
    let target = version_pr_target(args, &config, root)?;
    let token = target.as_ref().map(|(token, _)| token.as_str());
    let bumps: Vec<VersionBump> = plan
        .iter()
        .map(|entry| VersionBump {
            name: entry.name.clone(),
            from: entry.from.clone(),
            to: entry.to.clone(),
            bump: entry.bump.as_str().to_owned(),
        })
        .collect();
    let planned: Vec<(String, String)> = plan
        .iter()
        .map(|entry| (entry.name.clone(), entry.to.clone()))
        .collect();
    let served = versions_for_templates(root, &config, &planned)?;
    // Before a manifest moves: a stale pin must not leave
    // versions bumped with no changelog, no follow-up, and fragments
    // still on disk.
    let mut pinned = pins::plan(root, &config.pins, &planned)?;
    pinned.extend(templates::plan(
        root,
        &config.templates,
        &config.pins,
        &served,
    )?);
    let cwd = env::current_dir()
        .and_then(|dir| dir.canonicalize())
        .context("the working directory")?;
    let steps = prepare::steps(&plan, config.prepare.after.as_deref(), &cwd);
    let after: Vec<String> = steps.iter().map(prepare::Step::command).collect();
    if dry_run {
        let preview = Preview {
            bumps,
            pinned,
            after,
            open_pr: args.open_pr(),
        };
        return preview_report(root, &plan, &fragments, &recovered, preview);
    }
    let origins = release::origins_with(root, token, &config.changelog, &fragments, &recovered)?;
    let shown = shown_fragments(&recovered, &fragments);
    let changelog = release::changelog_sections_with(&plan, &shown, &origins)?;
    prepare::apply_plan(&plan)?;
    // The tag names the merge commit, so a pin that lands after publish
    // never reaches the tree consumers fetch by ref. Rewrite them here,
    // where they ride the Version PR. Only the versions this release
    // names: a pin on a package no fragment bumped stays where it is.
    let mut pinned = pins::write(root, &config.pins, &planned)?;
    // Served files are rendered from the templates beside them, on the same
    // commit, and staged from here: a repo never lists them in stage.
    pinned.extend(templates::write(
        root,
        &config.templates,
        &config.pins,
        &served,
    )?);
    let consumed = release::contributing_fragments(&plan, &fragments);
    let changelogs = release::write_changelogs_with(&config, root, &plan, &shown, &origins)?;
    prepare::run_steps(&steps)?;
    let consume = file_names(&consumed);
    let mut paths: Vec<PathBuf> = plan.iter().map(|entry| entry.path.clone()).collect();
    paths.extend(pinned.iter().cloned());
    paths.extend(changelogs);
    paths.extend(consumed.iter().map(|fragment| fragment.path.clone()));
    paths.extend(steps.iter().filter_map(|step| step.lockfile.clone()));
    let staged = git::assert_only_allowed(
        root,
        &paths,
        &config.prepare.stage,
        config.prepare.stage_ignored,
    )?;
    paths.extend(staged);
    release::consume_fragments(consumed)?;
    let mut pr = None;
    if let Some((token, version_pr)) = &target {
        let opened = release::open_or_update_pr(root, token, version_pr, &paths, &changelog)?;
        pr = opened.map(|opened| opened.url);
    }
    Ok(VersionReport {
        changelog,
        consume,
        pins: display_paths(pinned),
        pr,
        after,
        dry_run: false,
        bumps,
    })
}

/// What a section renders from: the fragments earlier candidates consumed,
/// oldest first, then the pending ones. Only the pending ones are consumed.
fn shown_fragments(
    recovered: &[release::Recovered],
    pending: &[fragment::Fragment],
) -> Vec<fragment::Fragment> {
    recovered
        .iter()
        .map(|recovered| recovered.fragment.clone())
        .chain(pending.iter().cloned())
        .collect()
}

/// The file name of each consumed fragment, as the report lists it.
fn file_names(fragments: &[&fragment::Fragment]) -> Vec<String> {
    fragments
        .iter()
        .map(|fragment| {
            fragment
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_owned()
        })
        .collect()
}

fn display_paths(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.display().to_string())
        .collect()
}

/// What `version --dry-run` reports beside the changelog it renders.
struct Preview {
    bumps: Vec<VersionBump>,
    pinned: Vec<PathBuf>,
    after: Vec<String>,
    open_pr: bool,
}

fn preview_report(
    root: &Path,
    plan: &[prepare::PlanEntry],
    fragments: &[fragment::Fragment],
    recovered: &[release::Recovered],
    preview: Preview,
) -> Result<VersionReport> {
    let Preview {
        bumps,
        pinned,
        after,
        open_pr,
    } = preview;
    let origins = release::origins_with(
        root,
        None,
        &config::Changelog::default(),
        fragments,
        recovered,
    )?;
    let changelog =
        release::changelog_sections_with(plan, &shown_fragments(recovered, fragments), &origins)?;
    let consume = if open_pr {
        fragments
            .iter()
            .map(|fragment| {
                fragment
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?")
                    .to_owned()
            })
            .collect()
    } else {
        Vec::new()
    };
    let pr = open_pr.then(|| preview_pr(root));
    Ok(VersionReport {
        bumps,
        changelog,
        consume,
        pins: display_paths(pinned),
        pr,
        after,
        dry_run: true,
    })
}

/// What `version --pr` would do to the Version PR, found without writing
/// anything. The branch needs the base, and the lookup needs a token.
fn preview_pr(root: &Path) -> String {
    let Ok(base) = github::base_branch(root) else {
        return formatdoc!(
            "open or update {prefix}<base> (base not resolved)",
            prefix = release::VERSION_BRANCH_PREFIX
        );
    };
    let branch = release::version_branch(&base);
    let Ok(token) = release::resolve_token() else {
        return formatdoc!("open or update {branch} (auth not checked)");
    };
    match github::repo(root).and_then(|repo| github::existing_pr(&token, &repo, &branch)) {
        Ok(Some(existing)) => formatdoc!("update {url}", url = existing.url),
        Ok(None) => formatdoc!("open {branch}"),
        Err(_) => formatdoc!("open or update {branch} (lookup failed)"),
    }
}

/// The registry token of a run that may create crates, and the notice a dry
/// run gives when `VERCTL_REGISTRY_TOKEN` is unset. A dry run uploads nothing,
/// so it needs no token. A real run fails without it, unless `defer` leaves
/// the failure to the caller, which names every token it lacks.
fn new_token(
    options: &ReleaseOptions,
    defer: bool,
) -> Result<(Option<RegistryToken>, Option<String>)> {
    match RegistryToken::from_env_new(&options.registry_token_env) {
        Ok(token) => Ok((Some(token), None)),
        Err(error) if options.dry_run() => {
            Ok((None, Some(format!("a real run would fail: {error:#}"))))
        }
        Err(_) if defer => Ok((None, None)),
        Err(error) => Err(error),
    }
}

/// What a release leaves out, declares and starts, from its options.
fn scope_of(options: &ReleaseOptions, new_crates: NewCrates) -> publish::Scope {
    publish::Scope {
        skip_assets: options.skip_assets,
        declared_assets: options.declared_assets.clone(),
        releases_only: options.releases_only,
        assets_dir: options.assets_dir.clone(),
        new_crates,
    }
}

/// The proofs a release may go past, from its options.
const fn allow_of(options: &ReleaseOptions) -> publish::Allow {
    publish::Allow {
        workflow_change: options.allow.allow_workflow_change,
        off_default_branch: options.allow.allow_off_default_branch,
    }
}

/// The crates.io the release reads, and the owner's token for its entries.
fn crates_io_of<'a>(
    options: &'a ReleaseOptions,
    owner_token: Option<&'a str>,
) -> registry::CratesIo<'a> {
    registry::CratesIo {
        index: &options.crates_io_index,
        api: &options.crates_io_api,
        owner_token,
    }
}

fn release_report(args: &ReleaseArgs) -> Result<ReleaseReport> {
    let (registry_token, token_notice) = if args.new_crates {
        new_token(args, false)?
    } else {
        (RegistryToken::from_env(&args.registry_token_env)?, None)
    };
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let new_crates = if args.new_crates {
        NewCrates::Every
    } else {
        NewCrates::Off
    };
    let owner_token = registry_token::owner_token();
    let run = publish::run_pending(
        &config,
        &args.config,
        root,
        args.dry_run(),
        &scope_of(args, new_crates),
        allow_of(args),
        registry_token.as_ref(),
        crates_io_of(args, owner_token.as_deref()),
    )?;
    Ok(release_report_of(
        run,
        args.dry_run(),
        token_notice,
        Vec::new(),
    ))
}

/// The report of a release that `run` made, with the notice a dry run gave
/// for a missing token, and the trusted publisher rows that followed it.
fn release_report_of(
    run: publish::PublishRun,
    dry_run: bool,
    token_notice: Option<String>,
    trust: Vec<first_publish::Row>,
) -> ReleaseReport {
    let notices: Vec<String> = token_notice
        .into_iter()
        .chain(run.shortfalls.iter().map(ToString::to_string))
        .collect();
    ReleaseReport {
        outcome: run.outcome,
        packages: run.published.packages,
        releases: run.published.releases,
        allowed: run.allowed,
        dry_run,
        notices,
        proof: run.proof,
        trust,
    }
}

/// `release bootstrap`: `release --new` for the crates named, or every crate
/// crates.io lacks, then `release trust` for each crate that release created.
/// A real run needs `VERCTL_REGISTRY_TOKEN` and `VERCTL_OWNER_TOKEN`, checked
/// before any write. `--dry-run` needs neither.
fn bootstrap_report(step: &BootstrapArgs, before: &ReleaseArgs) -> Result<ReleaseReport> {
    let after = AfterVerb::read(&env::args_os().collect::<Vec<_>>())?;
    let options = step.options.clone().under(&before.options, &after);
    let (registry_token, token_notice) = new_token(&options, true)?;
    let config = Config::load(&options.config)?;
    let root = &config::root_of(&options.config);
    let owner_token = registry_token::owner_token();
    let request = bootstrap::Request {
        names: &step.crates,
        dry_run: options.dry_run(),
        scope: scope_of(&options, NewCrates::Off),
        allow: allow_of(&options),
        registry_token: registry_token.as_ref(),
        crates_io: crates_io_of(&options, owner_token.as_deref()),
    };
    let (run, trust) = bootstrap::run(&config, &options.config, root, &request)?;
    Ok(release_report_of(
        run,
        options.dry_run(),
        token_notice,
        trust,
    ))
}

/// `release trust`: add this repository's trusted publisher entry to each
/// crate that lacks one. It needs `VERCTL_OWNER_TOKEN`, dry run included,
/// because the list of entries needs it.
fn trust_report(args: &TrustArgs, release: &ReleaseArgs) -> Result<CratesReport> {
    let path = args.config_under(release);
    let config = Config::load(path)?;
    let root = &config::root_of(path);
    let owner_token = registry_token::owner_token();
    let crates_io = registry::CratesIo {
        index: registry::CRATES_IO_INDEX,
        api: &args.crates_io_api,
        owner_token: owner_token.as_deref(),
    };
    let request = trust::Request::new(&args.crates, args.dry_run_under(release), crates_io);
    Ok(CratesReport {
        crates: trust::run(&config, root, &request)?,
    })
}

/// Every `pins` entry, then every template, at the versions the manifests
/// hold: the files `check` compares. Both are planned before either is
/// written. A pin that does not match, a template that fails, or a file both
/// claim therefore leaves every file unchanged.
fn render_report(args: &RenderArgs) -> Result<RenderReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let versions = pins::current_versions(root, &config)?;
    let served = versions_for_templates(root, &config, &[])?;
    let mut files = pins::plan(root, &config.pins, &versions)?;
    files.extend(templates::plan(
        root,
        &config.templates,
        &config.pins,
        &served,
    )?);
    if !args.dry_run() {
        pins::write(root, &config.pins, &versions)?;
        templates::write(root, &config.templates, &config.pins, &served)?;
    }
    Ok(RenderReport {
        files: display_paths(files),
    })
}

/// `release asset <target>`: build the target's tarball and upload nothing,
/// so the leg needs no token. `release --assets-dir` uploads it to the draft
/// after the run reads GitHub, and before any registry upload.
/// `--config` counts before or after `asset`.
fn asset_report(args: &AssetArgs, release: &ReleaseArgs) -> Result<AssetsReport> {
    let path = args.config_under(release);
    let config = Config::load(path)?;
    let root = &config::root_of(path);
    let planned = assets::plan(&config, root)?;
    let tarball = assets::build(&planned, &args.target, root, args.prebuilt.as_deref())?;
    Ok(AssetsReport {
        plan: planned,
        tarball: Some(tarball.display().to_string()),
        prebuilt: args
            .prebuilt
            .as_ref()
            .map(|path| path.display().to_string()),
    })
}

/// `release asset <target> --vendored <name>`: build the set's archive for
/// the target, then upload it with its checksum unless `--dry-run`.
fn vendored_report(args: &AssetArgs, release: &ReleaseArgs, name: &str) -> Result<VendoredReport> {
    let path = args.config_under(release);
    let config = Config::load(path)?;
    let root = &config::root_of(path);
    let planned = assets::plan_vendored(&config, name)?;
    let archive = assets::build_vendored(&planned, &args.target, root)?;
    let uploaded = if args.dry_run_under(release) {
        None
    } else {
        Some(assets::upload_vendored(&planned, root, &archive)?)
    };
    Ok(VendoredReport {
        plan: planned,
        archive: Some(archive.display().to_string()),
        uploaded,
    })
}

fn ci_report(args: &CiArgs) -> Result<CiReport> {
    let config = Config::load(&args.config)?;
    let planned = ci::plan(&config)?;
    if let Some(path) = &args.github_output {
        ci::write_github_output(&planned, path)?;
    }
    Ok(CiReport { plan: planned })
}

/// The fragments, the asset plan, and the incomplete releases at HEAD.
///
/// With `--github-output`, `status` fetches the full history and needs a
/// token, so the workflow's plan is never partial. It writes the asset
/// outputs (`has_assets`, `tag`, `matrix`) and the release outputs
/// (`commits`, `pending`, `assets`).
///
/// Under `--vendored`, `status` plans the set and nothing else: it writes only
/// the asset outputs, and fetches no history, reads no token, and looks up no
/// release.
///
/// A config with no packages has no package assets to plan.
///
/// Without it, `status` changes nothing in the clone and reads the history
/// already there. With no token, or when a lookup cannot run
/// ([`lookup::Unavailable`]), it lists the incomplete releases as not checked,
/// with the reason. It never lists them as none.
///
/// `status` also names the enforcement tier of the default branch's rules
/// ([`freeze::Tier`]). Reading it never fails `status`: a missing token or a
/// refused read shows the tier as unknown, with the reason.
///
/// Any other error, such as a release tag on the wrong commit, fails
/// `status` in both modes. Under `--github-output`, a lookup that cannot run
/// fails it too, since the workflow must not plan from a guess.
fn status_report(args: &StatusArgs) -> Result<StatusReport> {
    let fragments = fragment::load_dir(&args.dir)?;
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let assets = if let Some(name) = &args.vendored {
        let planned = assets::plan_vendored(&config, name)?;
        if let Some(path) = &args.github_output {
            assets::write_vendored_github_output(&planned, path)?;
        }
        Some(StatusAssets::Vendored(planned))
    } else if config.packages.is_empty() {
        None
    } else {
        let planned = assets::plan(&config, root)?;
        if let Some(path) = &args.github_output {
            assets::write_github_output(&planned, path)?;
        }
        planned
            .has_assets
            .then_some(StatusAssets::Packages(planned))
    };
    let Unreleased {
        rows: unreleased,
        unchecked: unreleased_unchecked,
        partial,
        notices,
    } = if args.vendored.is_some() {
        Unreleased::unchecked(Unchecked::vendored())
    } else {
        unreleased_state(args, &config, root)?
    };
    let tier = if args.vendored.is_some() {
        freeze::Tier::Unknown {
            reason: "a vendored plan reads no rules".to_owned(),
        }
    } else {
        freeze::Tier::read(root)
    };
    let overall = fragments
        .iter()
        .map(fragment::Fragment::max_bump)
        .max()
        .unwrap_or(Bump::None);
    Ok(StatusReport {
        pending: fragments.len(),
        max: overall.as_str().to_owned(),
        fragments: fragments
            .iter()
            .map(|fragment| StatusFragment {
                file: fragment
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?")
                    .to_owned(),
                max: fragment.max_bump().as_str().to_owned(),
                packages: fragment
                    .packages
                    .iter()
                    .map(|package| StatusPackage {
                        name: package.name.clone(),
                        bump: package.bump.as_str().to_owned(),
                    })
                    .collect(),
            })
            .collect(),
        assets,
        unreleased,
        unreleased_unchecked,
        partial,
        tier,
        notices,
    })
}

/// What `status` found about the releases HEAD still has to make.
struct Unreleased {
    /// Every package whose release at HEAD is incomplete, or `None` when not
    /// checked.
    rows: Option<Vec<ReleaseRow>>,
    /// Why `rows` was not checked.
    unchecked: Option<Unchecked>,
    /// Every release some of whose uploads are live, or `None` when not
    /// checked.
    partial: Option<Vec<PartialRow>>,
    /// One warning per published, immutable Release that lacks declared
    /// assets.
    notices: Vec<String>,
}

impl Unreleased {
    fn unchecked(reason: Unchecked) -> Self {
        Self {
            rows: None,
            unchecked: Some(reason),
            partial: None,
            notices: Vec::new(),
        }
    }
}

/// The incomplete releases at HEAD, or why they were not checked.
fn unreleased_state(args: &StatusArgs, config: &Config, root: &Path) -> Result<Unreleased> {
    let token = match &args.github_output {
        Some(_) => {
            publish::full_history(root)?;
            Some(release::resolve_token()?)
        }
        None => release::env_token(),
    };
    let new_crates = if args.new_crates {
        NewCrates::Every
    } else {
        NewCrates::Off
    };
    Ok(match token {
        Some(token) => {
            match pending::pending(
                config,
                &args.config,
                root,
                &token,
                args.skip_assets,
                &[],
                None,
                &args.crates_io_index,
                &new_crates,
                &github::ReleaseList::default(),
            ) {
                Ok(planned) => {
                    if let Some(path) = &args.github_output {
                        pending::write_github_output(&planned, path)?;
                    }
                    Unreleased {
                        rows: Some(unreleased_rows(&planned)),
                        unchecked: None,
                        partial: Some(partial_rows(&planned)),
                        notices: planned.shortfalls.iter().map(ToString::to_string).collect(),
                    }
                }
                Err(error) if args.github_output.is_none() && lookup::is_unavailable(&error) => {
                    Unreleased::unchecked(Unchecked::failed(&error))
                }
                Err(error) => return Err(error),
            }
        }
        None => Unreleased::unchecked(Unchecked::missing_token()),
    })
}

/// Every release some, but not all, of whose uploads are live.
fn partial_rows(planned: &pending::Pending) -> Vec<PartialRow> {
    planned
        .partials
        .iter()
        .map(|partial| PartialRow {
            tag: partial.tag.clone(),
            live: partial.live,
            total: partial.total,
            noun: partial.noun.clone(),
            stop: partial.stop.clone(),
        })
        .collect()
}

/// Every package whose release at HEAD is incomplete.
fn unreleased_rows(planned: &pending::Pending) -> Vec<ReleaseRow> {
    planned
        .commits
        .iter()
        .flat_map(|commit| {
            commit.packages.iter().map(|package| ReleaseRow {
                name: package.name.clone(),
                version: package.version.clone(),
                commit: commit.sha.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    use ctl_core::parser;
    use verctl::cli::Cli;

    #[test]
    fn cli_debug_assert() {
        parser::verify::<Cli>();
        let _ = Cli::command();
    }
}
