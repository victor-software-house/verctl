//! The `verctl` binary: parses the command line and runs the chosen command.

use std::path::Path;

use anyhow::{Context, Result, bail};
use ctl_core::prelude::*;
use verctl::cli::{
    AssetArgs, CheckArgs, Cli, Command, ReleaseArgs, ReleaseStep, RenderArgs, StatusArgs,
    VersionArgs,
};
use verctl::config::{self, Config};
use verctl::fragment::{self, Bump};
use verctl::{
    assets, ci, docsrs, git, github, pending, pins, prepare, process, publish, release,
    semver_check, templates, versions,
};

mod presentation;

use presentation::{
    AssetsReport, CheckReport, CiReport, DocsReport, InstructionsReport, ReleaseReport, ReleaseRow,
    RenderReport, Report, SemverReport, StatusAssets, StatusFragment, StatusPackage, StatusReport,
    VendoredReport, VersionBump, VersionCheckReport, VersionReport,
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
        Command::Check(args) if args.versions => versions_report(&args).map(Report::VersionCheck),
        Command::Check(args) => check_report(&args).map(Report::Check),
        Command::Version(args) => version_report(&args).map(Report::Version),
        Command::Release(args) => match &args.step {
            Some(ReleaseStep::Asset(asset)) => match &asset.vendored {
                Some(name) => vendored_report(asset, name).map(Report::Vendored),
                None => asset_report(asset).map(Report::Assets),
            },
            None => release_report(&args).map(Report::Release),
        },
        Command::Render(args) => render_report(&args).map(Report::Render),
        Command::Ci(args) => ci_report(&args).map(Report::Ci),
        Command::Docs(args) => docs_report(&args).map(Report::Docs),
        Command::Semver(args) => semver_report(&args).map(Report::Semver),
        Command::Removed(removed) => match removed {},
    }
}

fn semver_report(args: &verctl::cli::SemverArgs) -> Result<SemverReport> {
    let config = Config::load(&args.config)?;
    let fragments = fragment::load_dir(&args.dir)?;
    Ok(SemverReport {
        crates: semver_check::run(&config, &config::root_of(&args.config), &fragments)?,
    })
}

fn docs_report(args: &verctl::cli::DocsArgs) -> Result<DocsReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let timeout = std::time::Duration::from_secs(args.timeout.saturating_mul(60));
    Ok(DocsReport {
        crates: docsrs::run(&config, root, timeout)?,
    })
}

/// Fragments that parse and that `version` would accept: every package they
/// name is declared, and no bump they ask for is refused. Then the served
/// files, whether or not a fragment is pending, since a hand edit fails
/// here and not at the next release that overwrites it.
fn check_report(args: &CheckArgs) -> Result<CheckReport> {
    let fragments = fragment::load_dir(&args.dir)?;
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    if !fragments.is_empty() {
        prepare::plan(&config, &fragments, root)?;
    }
    check_served(root, &config)?;
    Ok(CheckReport {
        ok: fragments.len(),
    })
}

/// Every template target and `pins` file that differs from what verctl would
/// write at the versions the manifests hold, listed in one error so one run
/// names every file to fix.
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

fn versions_report(args: &CheckArgs) -> Result<VersionCheckReport> {
    let root = &config::root_of(&args.config);
    let config = Config::load(&args.config)?;
    let report = versions::report(root, &config)?;
    Ok(VersionCheckReport {
        skip: report.skip,
        rows: report.rows,
    })
}

/// The versions templates render from.
///
/// A pin moves only what this release names, but a template renders a whole
/// file: a served file that mentions a package no fragment bumped still has to
/// say that package's current version. That map costs every manifest a read, so
/// a repo that serves nothing from a template never builds it.
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
    if plan.is_empty() {
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
    if dry_run {
        return preview_report(root, &plan, &fragments, bumps, pinned, args.open_pr());
    }
    let follow_up = prepare::apply_plan(&plan)?;
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
    let origins = release::origins(root, token, &config.changelog, &fragments)?;
    let changelog = release::changelog_sections_with(&plan, &fragments, &origins)?;
    let changelogs = release::write_changelogs_with(&config, root, &plan, &fragments, &origins)?;
    if let Some(after) = &config.prepare.after {
        process::run_inherit(after, std::time::Duration::from_mins(5)).context("prepare.after")?;
    }
    let consume: Vec<String> = consumed
        .iter()
        .map(|fragment| {
            fragment
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_owned()
        })
        .collect();
    let mut paths: Vec<std::path::PathBuf> = plan.iter().map(|entry| entry.path.clone()).collect();
    paths.extend(pinned.iter().cloned());
    paths.extend(changelogs);
    paths.extend(consumed.iter().map(|fragment| fragment.path.clone()));
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
        after: follow_up,
        dry_run: false,
        bumps,
    })
}

fn display_paths(paths: Vec<std::path::PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.display().to_string())
        .collect()
}

fn preview_report(
    root: &Path,
    plan: &[prepare::PlanEntry],
    fragments: &[fragment::Fragment],
    bumps: Vec<VersionBump>,
    pinned: Vec<std::path::PathBuf>,
    open_pr: bool,
) -> Result<VersionReport> {
    let origins = release::origins(root, None, &config::Changelog::default(), fragments)?;
    let changelog = release::changelog_sections_with(plan, fragments, &origins)?;
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
        after: plan
            .iter()
            .filter_map(|entry| entry.driver.after().map(str::to_owned))
            .collect(),
        dry_run: true,
    })
}

/// What `version --pr` would do to the Version PR, as far as a preview can
/// tell without writing: the branch needs the base, and the lookup needs a
/// token.
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

fn release_report(args: &ReleaseArgs) -> Result<ReleaseReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let allow = publish::Allow {
        workflow_change: args.allow.allow_workflow_change,
        off_default_branch: args.allow.allow_off_default_branch,
    };
    let run = publish::run_pending(&config, root, args.dry_run(), args.skip_assets, allow)?;
    Ok(ReleaseReport {
        outcome: run.outcome,
        packages: run.published.packages,
        releases: run.published.releases,
        waiting: run.waiting.iter().map(ReleaseRow::from).collect(),
        allowed: run.allowed,
        dry_run: args.dry_run(),
    })
}

/// Every `pins` entry, then every template, at the versions the manifests
/// hold: the files `check` compares. Both are planned before either is
/// written, so a pin that no longer matches, a template that fails, or a file
/// both claim changes nothing.
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

/// `release asset <target>`: build the target's tarball, then upload it to
/// `--tag`, or the first package's tag, unless `--dry-run` asks for the build
/// alone.
fn asset_report(args: &AssetArgs) -> Result<AssetsReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let planned = assets::plan(&config, root)?;
    let tarball = assets::build(&planned, &args.target, root)?;
    let uploaded = if args.dry_run() {
        None
    } else {
        let tag = args.tag.as_deref().unwrap_or(planned.tag.as_str());
        Some(assets::upload(root, tag, &tarball)?)
    };
    Ok(AssetsReport {
        plan: planned,
        tarball: Some(tarball.display().to_string()),
        uploaded,
    })
}

/// `release asset <target> --vendored <name>`: build the set's archive for
/// the target, then upload it with its checksum unless `--dry-run`.
fn vendored_report(args: &AssetArgs, name: &str) -> Result<VendoredReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let planned = assets::plan_vendored(&config, name)?;
    let archive = assets::build_vendored(&planned, &args.target, root)?;
    let uploaded = if args.dry_run() {
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

fn ci_report(args: &verctl::cli::CiArgs) -> Result<CiReport> {
    let config = Config::load(&args.config)?;
    let planned = ci::plan(&config)?;
    if let Some(path) = &args.github_output {
        ci::write_github_output(&planned, path)?;
    }
    Ok(CiReport { plan: planned })
}

/// The fragments, the asset plan, and the incomplete version commits.
///
/// With `--github-output`, `status` fetches the full history and needs a
/// token, so the workflow's plan is never partial, and it writes the asset
/// outputs (`has_assets`, `tag`, `matrix`) and the release outputs
/// (`commits`, `pending`, `assets`). Without it, `status` changes nothing in
/// the clone: it reads the history already there, and with no token it
/// reports the version commits as not checked rather than as none.
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
    } else {
        let planned = assets::plan(&config, root)?;
        if let Some(path) = &args.github_output {
            assets::write_github_output(&planned, path)?;
        }
        planned
            .has_assets
            .then_some(StatusAssets::Packages(planned))
    };
    let token = match &args.github_output {
        Some(_) => {
            publish::full_history(root)?;
            Some(release::resolve_token()?)
        }
        None => release::env_token(),
    };
    let (unreleased, unreleased_unchecked) = match token {
        Some(token) => {
            let planned = pending::pending(&config, root, &token, args.skip_assets)?;
            if let Some(path) = &args.github_output {
                pending::write_github_output(&planned, path)?;
            }
            (Some(unreleased_rows(&planned)), None)
        }
        None => (None, Some("no GITHUB_TOKEN or GH_TOKEN".to_owned())),
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
    })
}

/// Every package at an incomplete version commit, oldest commit first.
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
    use verctl::cli::Cli;

    #[test]
    fn cli_debug_assert() {
        ctl_core::parser::verify::<Cli>();
        let _ = Cli::command();
    }
}
