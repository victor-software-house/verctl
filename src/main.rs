use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::process::ExitCode;

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
    }
    Ok(())
}
