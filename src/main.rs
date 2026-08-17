use anyhow::{Context, Result};
use ctl_core::prelude::*;
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;
use verctl::assets;
use verctl::cli::{CheckArgs, Cli, Command, PrepareArgs, PublishArgs, StatusArgs};
use verctl::config::Config;
use verctl::fragment::{self, Bump};
use verctl::git;
use verctl::github;
use verctl::pins;
use verctl::prepare;
use verctl::process;
use verctl::publish;
use verctl::release;
use verctl::versions;

const INSTRUCTIONS: &str = include_str!("instructions.md");

fn main() -> ExitCode {
    go::<Cli>("verctl", |cli| {
        let view = cli.format.view(cli.color.color());
        match cli.command {
            Command::Instructions => io::stdout().write_all(INSTRUCTIONS.as_bytes())?,
            Command::Status(args) => view.show(&status_report(&args)?)?,
            Command::Check(args) => {
                if args.versions {
                    let report = versions_report(&args)?;
                    view.show(&report)?;
                    report.require()?;
                } else {
                    let ok = fragment::load_dir(&args.dir)?.len();
                    view.show(&CheckReport { ok })?;
                }
            }
            Command::Prepare(args) => view.show(&prepare_report(&args)?)?,
            Command::Publish(args) => view.show(&publish_report(&args)?)?,
            Command::Pin(args) => view.show(&pin_report(&args)?)?,
            Command::Assets(args) => view.show(&assets_report(&args)?)?,
        }
        Ok(())
    })
}

#[derive(Serialize)]
struct CheckReport {
    ok: usize,
}

impl CheckReport {
    fn pretty(&self, color: ColorMode) -> String {
        kv(color, [("ok", format!("{n} fragment(s)", n = self.ok))])
    }
}

impl Render for CheckReport {
    fn render_pretty(&self) -> String {
        self.pretty(ColorMode::Always)
    }

    fn render_pretty_colored(&self, color: ColorMode) -> String {
        self.pretty(color)
    }
}

#[derive(Serialize)]
struct VersionCheckReport {
    skip: Option<String>,
    rows: Vec<versions::VersionRow>,
}

impl VersionCheckReport {
    fn pretty(&self, color: ColorMode) -> String {
        if let Some(skip) = &self.skip {
            return kv(color, [("exempt", skip.clone())]);
        }
        let drifted: Vec<&versions::VersionRow> =
            self.rows.iter().filter(|row| row.drifted()).collect();
        if drifted.is_empty() {
            return kv(color, [("versions", "match")]);
        }
        let rows: Vec<Vec<String>> = drifted
            .iter()
            .map(|row| {
                vec![
                    row.name.clone(),
                    row.remote.clone().unwrap_or_else(|| "-".into()),
                    row.local.clone(),
                ]
            })
            .collect();
        grid(color, &["name", "default", "local"], rows)
    }
}

impl Render for VersionCheckReport {
    fn render_pretty(&self) -> String {
        self.pretty(ColorMode::Always)
    }

    fn render_pretty_colored(&self, color: ColorMode) -> String {
        self.pretty(color)
    }
}

impl VersionCheckReport {
    fn require(&self) -> Result<()> {
        let report = versions::VersionReport {
            skip: self.skip.clone(),
            rows: self.rows.clone(),
        };
        report.require_clean()
    }
}

fn versions_report(args: &CheckArgs) -> Result<VersionCheckReport> {
    let root = args
        .config
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let config = Config::load(&args.config)?;
    let report = versions::report(root, &config)?;
    Ok(VersionCheckReport {
        skip: report.skip,
        rows: report.rows,
    })
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

impl StatusReport {
    fn pretty(&self, color: ColorMode) -> String {
        if self.pending == 0 {
            return kv(color, [("pending", "0")]);
        }
        let rows: Vec<Vec<String>> = self
            .fragments
            .iter()
            .flat_map(|fragment| {
                fragment.packages.iter().map(|package| {
                    vec![
                        fragment.file.clone(),
                        package.name.clone(),
                        package.bump.clone(),
                    ]
                })
            })
            .collect();
        let mut out = grid(color, &["file", "package", "bump"], rows);
        out.push('\n');
        out.push_str(&kv(
            color,
            [
                ("pending", self.pending.to_string()),
                ("max", self.max.clone()),
            ],
        ));
        out
    }
}

impl Render for StatusReport {
    fn render_pretty(&self) -> String {
        self.pretty(ColorMode::Always)
    }

    fn render_pretty_colored(&self, color: ColorMode) -> String {
        self.pretty(color)
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

impl PrepareReport {
    fn pretty(&self, color: ColorMode) -> String {
        if self.pr.as_deref() == Some("no-op") && self.bumps.is_empty() {
            return kv(color, [("no-op", "no version-changing fragments")]);
        }
        let mut out = String::new();
        if !self.bumps.is_empty() {
            let rows: Vec<Vec<String>> = self
                .bumps
                .iter()
                .map(|bump| {
                    vec![
                        bump.name.clone(),
                        bump.from.clone(),
                        bump.to.clone(),
                        bump.bump.clone(),
                    ]
                })
                .collect();
            out.push_str(&grid(color, &["name", "from", "to", "bump"], rows));
        }
        if !self.changelog.trim().is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(self.changelog.trim_end());
            out.push('\n');
        }
        let mut extra: Vec<(&str, String)> = Vec::new();
        for file in &self.consume {
            extra.push(("consume", file.clone()));
        }
        if let Some(pr) = &self.pr {
            extra.push(("pr", pr.clone()));
        }
        for cmd in &self.next {
            extra.push(("next", cmd.clone()));
        }
        if self.dry_run {
            extra.push(("dry-run", "nothing written".into()));
        }
        if !extra.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&kv(color, extra));
        }
        out
    }
}

impl Render for PrepareReport {
    fn render_pretty(&self) -> String {
        self.pretty(ColorMode::Always)
    }

    fn render_pretty_colored(&self, color: ColorMode) -> String {
        self.pretty(color)
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
    paths.extend(changelogs);
    paths.extend(consumed.iter().map(|fragment| fragment.path.clone()));
    paths.extend(git::stage_matches(root, &config.prepare.stage)?);
    git::assert_only_allowed(root, &paths, &config.prepare.stage)?;
    release::consume_fragments(consumed)?;
    let mut pr = None;
    if let Some(token) = token {
        let title = std::env::var("VERCTL_PR_TITLE")
            .unwrap_or_else(|_| "chore(release): version packages".into());
        let message = std::env::var("VERCTL_COMMIT_MESSAGE").unwrap_or_else(|_| title.clone());
        pr = release::open_or_update_pr(root, &token, &title, &message, &paths, &changelog)?;
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
    packages: Vec<publish::PublishLine>,
    release: Option<String>,
    dry_run: bool,
}

impl PublishReport {
    fn pretty(&self, color: ColorMode) -> String {
        if self.packages.is_empty() && self.release.is_none() {
            return kv(color, [("no-op", "nothing to publish")]);
        }
        let with_notes = self.packages.iter().any(|entry| entry.note.is_some());
        let headers: &[&str] = if with_notes {
            &["name", "version", "via", "note"]
        } else {
            &["name", "version", "via"]
        };
        let rows: Vec<Vec<String>> = self
            .packages
            .iter()
            .map(|entry| {
                let mut row = vec![entry.name.clone(), entry.version.clone(), entry.via.clone()];
                if with_notes {
                    row.push(entry.note.clone().unwrap_or_default());
                }
                row
            })
            .collect();
        let mut out = grid(color, headers, rows);
        let mut extra = Vec::new();
        if let Some(url) = &self.release {
            extra.push(("release", url.as_str()));
        }
        if self.dry_run {
            extra.push(("dry-run", "nothing published"));
        }
        if !extra.is_empty() {
            out.push('\n');
            out.push_str(&kv(color, extra));
        }
        out
    }
}

impl Render for PublishReport {
    fn render_pretty(&self) -> String {
        self.pretty(ColorMode::Always)
    }

    fn render_pretty_colored(&self, color: ColorMode) -> String {
        self.pretty(color)
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
        packages: outcome.packages,
        release: outcome.release,
        dry_run: args.dry_run(),
    })
}

#[derive(Serialize)]
struct PinReport {
    files: Vec<String>,
}

impl PinReport {
    fn pretty(&self, color: ColorMode) -> String {
        if self.files.is_empty() {
            return kv(color, [("pins", "none")]);
        }
        kv(color, self.files.iter().map(|file| ("pin", file.clone())))
    }
}

impl Render for PinReport {
    fn render_pretty(&self) -> String {
        self.pretty(ColorMode::Always)
    }

    fn render_pretty_colored(&self, color: ColorMode) -> String {
        self.pretty(color)
    }
}

fn pin_report(args: &PublishArgs) -> Result<PinReport> {
    let config = Config::load(&args.config)?;
    let root = args
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let versions = pins::current_versions(root, &config)?;
    let files = if args.dry_run() {
        pins::plan(root, &config.pins, &versions)?
    } else {
        pins::write(root, &config.pins, &versions)?
    };
    Ok(PinReport {
        files: files
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
    })
}

#[derive(Serialize)]
struct AssetsReport {
    #[serde(flatten)]
    plan: assets::AssetsPlan,
    tarball: Option<String>,
    uploaded: Option<String>,
}

impl AssetsReport {
    fn pretty(&self, color: ColorMode) -> String {
        if !self.plan.has_assets {
            return kv(
                color,
                [("assets", "none (library or one host build is enough)")],
            );
        }
        let rows: Vec<Vec<String>> = self
            .plan
            .matrix
            .include
            .iter()
            .map(|row| vec![row.id.clone(), row.runner.clone(), row.asset.clone()])
            .collect();
        let mut out = grid(color, &["target", "runner", "asset"], rows);
        let mut extra = vec![("tag", self.plan.tag.as_str())];
        if let Some(path) = &self.tarball {
            extra.push(("tarball", path.as_str()));
        }
        if let Some(url) = &self.uploaded {
            extra.push(("upload", url.as_str()));
        }
        out.push('\n');
        out.push_str(&kv(color, extra));
        out
    }
}

impl Render for AssetsReport {
    fn render_pretty(&self) -> String {
        self.pretty(ColorMode::Always)
    }

    fn render_pretty_colored(&self, color: ColorMode) -> String {
        self.pretty(color)
    }
}

fn assets_report(args: &verctl::cli::AssetsArgs) -> Result<AssetsReport> {
    let config = Config::load(&args.config)?;
    let root = args
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
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
