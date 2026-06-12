use anyhow::{anyhow, Result};
use crate::core::state::KmgrState;
use colored::Colorize;
use tokio::fs;

/// Enables a deployed package.
///
/// Refactors the targeted package by stripping the `.disabled` suffix from its
/// artifact and updating its registry state to active.
pub async fn do_cmd(mod_name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;
    state.check_initialized()?;
    
    if let Some(id) = state.find_mod_id(&mod_name) {
        let (current_filename, was_enabled) = {
            let mod_info = state.installed_mods.get(&id).unwrap();
            (mod_info.filename.clone(), mod_info.enabled)
        };
        
        if was_enabled {
            println!("{} Mod '{}' is already enabled.", "::".cyan().bold(), mod_name.green());
            return Ok(());
        }
        
        let disabled_path = state.get_mod_path(&current_filename, false);
        let enabled_path = state.get_mod_path(&current_filename, true);
        
        if tokio::fs::try_exists(&disabled_path).await? {
            if let Err(e) = fs::rename(&disabled_path, &enabled_path).await {
                eprintln!("{} Failed to enable mod (rename failed): {}", "Error:".red().bold(), e);
                return Err(anyhow!("Rename failed"));
            }
        } else {
            println!("{} Warning: {} was not found, but marking as enabled in config.", "⚠".yellow(), disabled_path.bright_black());
        }
        
        if let Some(mod_info) = state.installed_mods.get_mut(&id) {
            mod_info.enabled = true;
        }
        
        state.save().await?;
        println!("{} {} enabled.", "✔".green(), mod_name.cyan());
    } else {
        println!("{} Mod '{}' not found in installed list.", "⚠".yellow(), mod_name.magenta());
    }

    Ok(())
}
