use anyhow::Result;
use tokio::fs;
use crate::core::state::KmgrState;
use colored::Colorize;

/// Initializes the application environment.
///
/// Takes the Minecraft version and the mod loader type as strings.
/// Configures the base state, provisions necessary directories, and writes
/// the initial configuration to disk.
pub async fn do_cmd(mc_version: String, loader: String) -> Result<()> {
    println!("{} Initialize kmgr environment...", "".cyan().bold());
    
    let mut state = KmgrState::load().await?;
    state.default_mc_version = mc_version;
    state.mod_loader = loader;
    
    fs::create_dir_all(&state.mods_folder).await?;
    println!("   {} Created `{}` directory", "✔".green(), state.mods_folder);

    state.save().await?;
    println!("   {} Initialized `kmgr.toml` (MC: {}, Loader: {})", "✔".green(), state.default_mc_version.yellow(), state.mod_loader.magenta());

    println!("\n{} {}", "=>".cyan().bold(), "Ready.".bright_black());
    Ok(())
}
