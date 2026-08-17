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
        formatdoc!("ok      {n} fragment(s)", n = self.ok)
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
                        formatdoc!(
                            "  {name:<32} {bump:<6} {file}",
                            name = package.name,
                            bump = package.bump,
                            file = file,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut lines = pkgs;
                lines.push(formatdoc!("    max {max}", max = max));
                lines
            })
            .collect::<Vec<_>>()
            .join("\n");
        formatdoc! {"
            pending  {pending}
            {rows}
            max     {max}
            ",
            pending = self.pending,
            rows = rows,
            max = self.max,
        }
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
                        formatdoc!(
                            "bump    {name}  {from} -> {to}  ({kind})",
                            name = bump.name,
                            from = bump.from,
                            to = bump.to,
                            kind = bump.bump,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if !self.changelog.is_empty() {
            blocks.push(
                self.changelog
                    .lines()
                    .map(|line| formatdoc!("log     {line}", line = line))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if !self.consume.is_empty() {
            blocks.push(
                self.consume
                    .iter()
                    .map(|file| formatdoc!("consume {file}", file = file))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if let Some(pr) = &self.pr {
            blocks.push(formatdoc!("pr      {pr}", pr = pr));
        }
        if !self.next.is_empty() {
            blocks.push(
                self.next
                    .iter()
                    .map(|cmd| formatdoc!("next    {cmd}", cmd = cmd))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if self.dry_run {
            blocks.push(formatdoc!("dry-run (no files written)"));
        }
        blocks.join("\n")
    }
}

fn prepare_report(args: &PrepareArgs) -> Result<PrepareReport> {
    let dry_run = args.dry_run();
    let token = if args.open_pr() && !dry_run {
        Some(release::resolve_token()?)
    } else {
        None
    };
    let config = Config::load(&args.config)?;
    let fragments = fragment::load_dir(&args.dir)?;
    let root = args
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let plan = prepare::plan(&config, &fragments, root)?;
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
    let mut pr = None;
    if let Some(token) = token {
        release::write_changelogs(&root.join("CHANGELOG.md"), &plan, &fragments)?;
        release::consume_fragments(&fragments)?;
        let title = std::env::var("VERCTL_PR_TITLE")
            .unwrap_or_else(|_| "chore(release): version packages".into());
        let message = std::env::var("VERCTL_COMMIT_MESSAGE").unwrap_or_else(|_| title.clone());
        pr = Some(release::open_or_update_pr(root, &token, &title, &message)?);
    }
    Ok(PrepareReport {
        changelog: String::new(),
        consume: Vec::new(),
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
