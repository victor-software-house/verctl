use anyhow::Result;
use clap::Parser;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use verctl::cli::{Cli, Command, PrepareArgs, StatusArgs};
use verctl::config::Config;
use verctl::fragment::{self, Bump};
use verctl::prepare;

const INSTRUCTIONS: &str = include_str!("instructions.md");

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
