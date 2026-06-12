mod cli;
mod commands;
mod clients;
mod core;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::exec(cli).await
}
