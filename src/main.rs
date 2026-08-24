use anyhow::{Context, Result};
use ctl_core::prelude::*;
use std::path::Path;
use verctl::assets;
use verctl::ci;
use verctl::cli::{CheckArgs, Cli, Command, PrepareArgs, PublishArgs, StatusArgs};
use verctl::config::{self, Config};
use verctl::fragment::{self, Bump};
use verctl::git;
use verctl::github;
use verctl::pins;
use verctl::prepare;
use verctl::process;
use verctl::publish;
use verctl::release;
use verctl::templates;
use verctl::versions;

mod presentation;

use presentation::{
    AssetsReport, CheckReport, CiReport, InstructionsReport, PinReport, PrepareBump, PrepareReport,
    PublishReport, Report, StatusFragment, StatusPackage, StatusReport, VersionCheckReport,
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
        Command::Check(args) => {
            let ok = fragment::load_dir(&args.dir)?.len();
            Ok(Report::Check(CheckReport { ok }))
        }
        Command::Prepare(args) => prepare_report(&args).map(Report::Prepare),
        Command::Publish(args) => publish_report(&args).map(Report::Publish),
        Command::Pin(args) => pin_report(&args).map(Report::Pin),
        Command::Assets(args) => assets_report(&args).map(Report::Assets),
        Command::Ci(args) => ci_report(&args).map(Report::Ci),
    }
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

fn prepare_report(args: &PrepareArgs) -> Result<PrepareReport> {
    let dry_run = args.dry_run();
    let config = Config::load(&args.config)?;
    let fragments = fragment::load_dir(&args.dir)?;
    let root = &config::root_of(&args.config);
    let plan = prepare::plan(&config, &fragments, root)?;
    if plan.is_empty() {
        return Ok(PrepareReport {
            bumps: Vec::new(),
            changelog: String::new(),
            consume: Vec::new(),
            pins: Vec::new(),
            pr: args.open_pr().then(|| "no-op".to_owned()),
            next: Vec::new(),
            dry_run,
        });
    }
    let token = if args.open_pr() && !dry_run {
        Some(release::resolve_token()?)
    } else {
        None
    };
    let bumps: Vec<PrepareBump> = plan
        .iter()
        .map(|entry| PrepareBump {
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
    pinned.extend(templates::plan(root, &config.templates, &served)?);
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
    // commit, and staged from here — a repo never lists them in stage.
    pinned.extend(templates::write(root, &config.templates, &served)?);
    let consumed = release::contributing_fragments(&plan, &fragments);
    let changelog = release::changelog_sections(&plan, &fragments)?;
    let changelogs = release::write_changelogs(&config, root, &plan, &fragments)?;
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
    if let Some(token) = token {
        let title = std::env::var("VERCTL_PR_TITLE")
            .unwrap_or_else(|_| "chore(release): version packages".into());
        let message = std::env::var("VERCTL_COMMIT_MESSAGE").unwrap_or_else(|_| title.clone());
        pr = release::open_or_update_pr(
            root,
            &token,
            &title,
            &message,
            &paths,
            &changelog,
            config.prepare.version_label(),
        )?;
    }
    Ok(PrepareReport {
        changelog,
        consume,
        pins: display_paths(pinned),
        pr,
        next: follow_up,
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
    bumps: Vec<PrepareBump>,
    pinned: Vec<std::path::PathBuf>,
    open_pr: bool,
) -> Result<PrepareReport> {
    let changelog = release::changelog_sections(plan, fragments)?;
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
    let pr = if open_pr {
        Some(match release::resolve_token() {
            Ok(token) => match github::repo(root)
                .and_then(|repo| github::existing_pr(&token, &repo, release::VERSION_BRANCH))
            {
                Ok(Some(existing)) => formatdoc!("update {url}", url = existing.url),
                Ok(None) => formatdoc!("open {branch}", branch = release::VERSION_BRANCH),
                Err(_) => formatdoc!(
                    "open or update {branch} (lookup failed)",
                    branch = release::VERSION_BRANCH
                ),
            },
            Err(_) => formatdoc!(
                "open or update {branch} (auth not checked)",
                branch = release::VERSION_BRANCH
            ),
        })
    } else {
        None
    };
    Ok(PrepareReport {
        bumps,
        changelog,
        consume,
        pins: display_paths(pinned),
        pr,
        next: plan
            .iter()
            .filter_map(|entry| entry.driver.after().map(str::to_owned))
            .collect(),
        dry_run: true,
    })
}

fn publish_report(args: &verctl::cli::PublishArgs) -> Result<PublishReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let outcome = publish::run(&config, root, args.dry_run())?;
    Ok(PublishReport {
        packages: outcome.packages,
        releases: outcome.releases,
        dry_run: args.dry_run(),
    })
}

fn pin_report(args: &PublishArgs) -> Result<PinReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let versions = pins::current_versions(root, &config)?;
    let files = if args.dry_run() {
        pins::plan(root, &config.pins, &versions)?
    } else {
        pins::write(root, &config.pins, &versions)?
    };
    Ok(PinReport {
        files: display_paths(files),
    })
}

fn assets_report(args: &verctl::cli::AssetsArgs) -> Result<AssetsReport> {
    let config = Config::load(&args.config)?;
    let root = &config::root_of(&args.config);
    let planned = assets::plan(&config, root)?;
    if let Some(path) = &args.github_output {
        assets::write_github_output(&planned, path)?;
    }
    if let Some(id) = &args.build {
        let tarball = assets::build(&planned, id, root)?;
        let uploaded = if args.upload {
            let tag = args.tag.as_deref().unwrap_or(planned.tag.as_str());
            Some(assets::upload(root, tag, &tarball)?)
        } else {
            None
        };
        return Ok(AssetsReport {
            plan: planned,
            tarball: Some(tarball.display().to_string()),
            uploaded,
        });
    }
    anyhow::ensure!(!args.upload, "--upload requires --build");
    Ok(AssetsReport {
        plan: planned,
        tarball: None,
        uploaded: None,
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

fn status_report(args: &StatusArgs) -> Result<StatusReport> {
    let fragments = fragment::load_dir(&args.dir)?;
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
    })
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
