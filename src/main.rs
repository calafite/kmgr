mod cli;
mod clients;
mod commands;
mod core;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

/// The main entry point of the application.
/// Parses command-line arguments and executes the corresponding command.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::exec(cli).await
}
