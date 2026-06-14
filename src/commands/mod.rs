pub mod disable;
pub mod enable;
pub mod init;
pub mod install;
pub mod list;
pub mod profile;
pub mod prune;
pub mod remove;
pub mod search;
pub mod setup;
pub mod update;

pub mod sync;

use crate::cli::{Cli, Commands};
use anyhow::Result;

/// Evaluates cli commands.
///
/// Takes the parsed Cli options and runs the mapped command routine.
pub async fn exec(cli: Cli) -> Result<()> {
    let jobs = cli.jobs;
    if jobs == 0 {
        anyhow::bail!("Jobs limit must be at least 1");
    }

    match cli.command {
        Commands::Init {
            mc_version,
            loader,
            mods_folder,
        } => init::do_cmd(mc_version, loader, mods_folder).await,
        Commands::Setup => setup::do_cmd().await,
        Commands::Search { query, source } => {
            let full_query = query.join(" ");
            search::do_cmd(full_query, source).await
        }
        Commands::Install {
            mods,
            mc_version,
            source,
        } => {
            for mod_name in mods {
                install::do_cmd(mod_name, mc_version.clone(), source.clone(), jobs).await?;
            }
            Ok(())
        }
        Commands::Update { apply } => update::do_cmd(apply, jobs).await,
        Commands::Remove { mods } => {
            for mod_name in mods {
                remove::do_cmd(mod_name).await?;
            }
            Ok(())
        }
        Commands::Enable { mods } => {
            for mod_name in mods {
                enable::do_cmd(mod_name).await?;
            }
            Ok(())
        }
        Commands::Disable { mods } => {
            for mod_name in mods {
                disable::do_cmd(mod_name).await?;
            }
            Ok(())
        }
        Commands::Sync => sync::do_cmd(jobs).await,
        Commands::Prune => prune::do_cmd().await,
        Commands::Profile { command } => profile::do_cmd(command).await,
        Commands::List => list::do_cmd().await,
    }
}
