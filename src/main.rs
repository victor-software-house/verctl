use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use verctl::config::Config;
use verctl::fragment::{self, Bump};
use verctl::prepare;

const INSTRUCTIONS: &str = include_str!("instructions.md");

#[derive(Parser)]
#[command(
    version,
    about = "Stack-agnostic version PRs from Changesets-format fragments",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the installed-version operator contract.
    Instructions,
    /// List pending .changeset fragments.
    Status(StatusArgs),
    /// Validate every fragment in a directory (fail closed).
    Check(StatusArgs),
    /// Apply fragment bumps to declared version files. Does not open a PR.
    Prepare(PrepareArgs),
}

#[derive(clap::Args)]
struct StatusArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset")]
    dir: PathBuf,
}

#[derive(clap::Args)]
struct PrepareArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset")]
    dir: PathBuf,
    /// Package map. Defaults to verctl.toml in the current directory.
    #[arg(short = 'c', long, default_value = "verctl.toml")]
    config: PathBuf,
    /// Print the plan and leave files alone.
    #[arg(long)]
    dry_run: bool,
    /// Local only. The Version PR lands in a later slice.
    #[arg(long, default_value_t = true)]
    no_pr: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("verctl: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Command::Instructions => {
            io::stdout().write_all(INSTRUCTIONS.as_bytes())?;
        }
        Command::Status(args) => print_status(&args)?,
        Command::Check(args) => {
            let fragments = fragment::load_dir(&args.dir)?;
            println!("ok      {} fragment(s)", fragments.len());
        }
        Command::Prepare(args) => prepare_local(&args)?,
    }
    Ok(())
}

fn prepare_local(args: &PrepareArgs) -> Result<()> {
    let _ = args.no_pr;
    let config = Config::load(&args.config)?;
    let fragments = fragment::load_dir(&args.dir)?;
    let root = args
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let plan = prepare::plan(&config, &fragments, root)?;
    for entry in &plan {
        println!(
            "bump    {}  {} -> {}  ({})",
            entry.name,
            entry.from,
            entry.to,
            entry.bump.as_str()
        );
    }
    if args.dry_run {
        println!("dry-run (no files written)");
        return Ok(());
    }
    let follow_up = prepare::apply_plan(&plan)?;
    for cmd in follow_up {
        println!("next    {cmd}");
    }
    Ok(())
}

fn print_status(args: &StatusArgs) -> Result<()> {
    let fragments = fragment::load_dir(&args.dir)?;
    if fragments.is_empty() {
        println!("pending  0");
        return Ok(());
    }
    println!("pending  {}", fragments.len());
    for fragment in &fragments {
        let file = fragment
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?");
        for package in &fragment.packages {
            println!(
                "  {:<32} {:<6} {}",
                package.name,
                package.bump.as_str(),
                file
            );
        }
        println!("    max {}", fragment.max_bump().as_str());
    }
    let overall = fragments
        .iter()
        .map(fragment::Fragment::max_bump)
        .max()
        .unwrap_or(Bump::None);
    println!("max     {}", overall.as_str());
    Ok(())
}
