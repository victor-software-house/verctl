use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use verctl::fragment::{self, Bump};

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
}

#[derive(clap::Args)]
struct StatusArgs {
    /// Directory of fragments. Defaults to .changeset.
    #[arg(short = 'd', long, default_value = ".changeset")]
    dir: PathBuf,
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
