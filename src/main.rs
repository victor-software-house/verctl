use anyhow::Result;
use ctl_core::prelude::*;
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;
use verctl::cli::{Cli, Command, PrepareArgs, StatusArgs};
use verctl::config::Config;
use verctl::fragment::{self, Bump};
use verctl::github;
use verctl::prepare;
use verctl::publish;
use verctl::release;

const INSTRUCTIONS: &str = include_str!("instructions.md");

fn main() -> ExitCode {
    go::<Cli>("verctl", |cli| {
        let view = cli.format.view(cli.color.color());
        match cli.command {
            Command::Instructions => io::stdout().write_all(INSTRUCTIONS.as_bytes())?,
            Command::Status(args) => view.show(&status_report(&args)?)?,
            Command::Check(args) => {
                let ok = fragment::load_dir(&args.dir)?.len();
                view.show(&CheckReport { ok })?;
            }
            Command::Prepare(args) => view.show(&prepare_report(&args)?)?,
            Command::Publish(args) => view.show(&publish_report(&args)?)?,
        }
        Ok(())
    })
}

#[derive(Serialize)]
struct CheckReport {
    ok: usize,
}

impl Render for CheckReport {
    fn render_pretty(&self) -> String {
        let n = self.ok;
        formatdoc!("ok      {n} fragment(s)")
    }
}

#[derive(Serialize)]
struct StatusReport {
    pending: usize,
    max: String,
    fragments: Vec<StatusFragment>,
}

#[derive(Serialize)]
struct StatusFragment {
    file: String,
    max: String,
    packages: Vec<StatusPackage>,
}

#[derive(Serialize)]
struct StatusPackage {
    name: String,
    bump: String,
}

impl Render for StatusReport {
    fn render_pretty(&self) -> String {
        if self.pending == 0 {
            return formatdoc!("pending  0");
        }
        let rows = self
            .fragments
            .iter()
            .flat_map(|fragment| {
                let file = fragment.file.as_str();
                let max = fragment.max.as_str();
                let pkgs = fragment
                    .packages
                    .iter()
                    .map(|package| {
                        let name = package.name.as_str();
                        let bump = package.bump.as_str();
                        formatdoc!("  {name:<32} {bump:<6} {file}")
                    })
                    .collect::<Vec<_>>();
                let mut lines = pkgs;
                lines.push(formatdoc!("    max {max}"));
                lines
            })
            .collect::<Vec<_>>()
            .join("\n");
        let pending = self.pending;
        let max = self.max.as_str();
        formatdoc! {"
            pending  {pending}
            {rows}
            max     {max}
        "}
    }
}

#[derive(Clone, Serialize)]
struct PrepareReport {
    bumps: Vec<PrepareBump>,
    changelog: String,
    consume: Vec<String>,
    pr: Option<String>,
    next: Vec<String>,
    dry_run: bool,
}

#[derive(Clone, Serialize)]
struct PrepareBump {
    name: String,
    from: String,
    to: String,
    bump: String,
}

impl Render for PrepareReport {
    fn render_pretty(&self) -> String {
        let mut blocks = Vec::new();
        if !self.bumps.is_empty() {
            blocks.push(
                self.bumps
                    .iter()
                    .map(|bump| {
                        let name = bump.name.as_str();
                        let from = bump.from.as_str();
                        let to = bump.to.as_str();
                        let kind = bump.bump.as_str();
                        formatdoc!("bump    {name}  {from} -> {to}  ({kind})")
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if !self.changelog.is_empty() {
            blocks.push(
                self.changelog
                    .lines()
                    .map(|line| formatdoc!("log     {line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if !self.consume.is_empty() {
            blocks.push(
                self.consume
                    .iter()
                    .map(|file| formatdoc!("consume {file}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if let Some(pr) = &self.pr {
            blocks.push(formatdoc!("pr      {pr}"));
        }
        if !self.next.is_empty() {
            blocks.push(
                self.next
                    .iter()
                    .map(|cmd| formatdoc!("next    {cmd}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if self.dry_run {
            blocks.push(formatdoc!("dry-run (no files written)"));
        }
        if self.pr.as_deref() == Some("no-op") && self.bumps.is_empty() {
            return formatdoc!("no-op   no version-changing fragments");
        }
        blocks.join("\n")
    }
}

fn prepare_report(args: &PrepareArgs) -> Result<PrepareReport> {
    let dry_run = args.dry_run();
    let config = Config::load(&args.config)?;
    let fragments = fragment::load_dir(&args.dir)?;
    let root = args
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let plan = prepare::plan(&config, &fragments, root)?;
    if plan.is_empty() {
        return Ok(PrepareReport {
            bumps: Vec::new(),
            changelog: String::new(),
            consume: Vec::new(),
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
    if dry_run {
        return preview_report(root, &plan, &fragments, bumps, args.open_pr());
    }
    let follow_up = prepare::apply_plan(&plan)?;
    let consumed = release::contributing_fragments(&plan, &fragments);
    let changelog = if args.open_pr() {
        release::changelog_sections(&plan, &fragments)?
    } else {
        String::new()
    };
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
    let mut pr = None;
    if let Some(token) = token {
        release::write_changelogs(&root.join("CHANGELOG.md"), &plan, &fragments)?;
        let mut paths: Vec<std::path::PathBuf> =
            plan.iter().map(|entry| entry.path.clone()).collect();
        paths.push(root.join("CHANGELOG.md"));
        paths.extend(consumed.iter().map(|fragment| fragment.path.clone()));
        release::consume_fragments(consumed)?;
        let title = std::env::var("VERCTL_PR_TITLE")
            .unwrap_or_else(|_| "chore(release): version packages".into());
        let message = std::env::var("VERCTL_COMMIT_MESSAGE").unwrap_or_else(|_| title.clone());
        pr = release::open_or_update_pr(root, &token, &title, &message, &paths)?;
    }
    Ok(PrepareReport {
        changelog,
        consume,
        pr,
        next: follow_up,
        dry_run: false,
        bumps,
    })
}

fn preview_report(
    root: &Path,
    plan: &[prepare::PlanEntry],
    fragments: &[fragment::Fragment],
    bumps: Vec<PrepareBump>,
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
                Ok(Some(url)) => format!("update {url}"),
                Ok(None) => format!("open {}", release::VERSION_BRANCH),
                Err(_) => format!("open or update {} (lookup failed)", release::VERSION_BRANCH),
            },
            Err(_) => format!(
                "open or update {} (auth not checked)",
                release::VERSION_BRANCH
            ),
        })
    } else {
        None
    };
    Ok(PrepareReport {
        bumps,
        changelog,
        consume,
        pr,
        next: plan
            .iter()
            .filter_map(|entry| entry.driver.after().map(str::to_owned))
            .collect(),
        dry_run: true,
    })
}

#[derive(Serialize)]
struct PublishReport {
    crates: Vec<String>,
    release: Option<String>,
    dry_run: bool,
}

impl Render for PublishReport {
    fn render_pretty(&self) -> String {
        if self.crates.is_empty() && self.release.is_none() {
            return formatdoc!("no-op   nothing to publish");
        }
        let crates = self
            .crates
            .iter()
            .map(|entry| format!("crate   {entry}"))
            .collect::<Vec<_>>()
            .join("\n");
        let release = self
            .release
            .as_deref()
            .map(|url| format!("release {url}"))
            .unwrap_or_default();
        let dry = if self.dry_run {
            "dry-run (nothing published)"
        } else {
            ""
        };
        [crates.as_str(), release.as_str(), dry]
            .into_iter()
            .filter(|block| !block.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn publish_report(args: &verctl::cli::PublishArgs) -> Result<PublishReport> {
    let config = Config::load(&args.config)?;
    let root = args
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let outcome = publish::run(&config, root, args.dry_run())?;
    Ok(PublishReport {
        crates: outcome.crates,
        release: outcome.release,
        dry_run: args.dry_run(),
    })
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
