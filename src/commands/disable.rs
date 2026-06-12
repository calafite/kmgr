use anyhow::{anyhow, Result};
use crate::core::state::KmgrState;
use colored::Colorize;
use tokio::fs;

/// Suspends execution of an active package.
///
/// Renames the target artifact by appending `.disabled` to its filename and
/// updates the local state to prevent it from loading.
pub async fn do_cmd(mod_name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;
    state.check_initialized()?;
    
    if let Some(id) = state.find_mod_id(&mod_name) {
        let (current_filename, was_enabled) = {
            let mod_info = state.installed_mods.get(&id).unwrap();
            (mod_info.filename.clone(), mod_info.enabled)
        };
        
        if !was_enabled {
            println!("{} Mod '{}' is already disabled.", "::".cyan().bold(), mod_name.green());
            return Ok(());
        }
        
        let enabled_path = state.get_mod_path(&current_filename, true);
        let disabled_path = state.get_mod_path(&current_filename, false);
        
        if tokio::fs::try_exists(&enabled_path).await? {
            if let Err(e) = fs::rename(&enabled_path, &disabled_path).await {
                eprintln!("{} Failed to disable mod (rename failed): {}", "Error:".red().bold(), e);
                return Err(anyhow!("Rename failed"));
            }
        } else {
            println!("{} Warning: {} was not found, but marking as disabled in config.", "⚠".yellow(), enabled_path.bright_black());
        }
        
        if let Some(mod_info) = state.installed_mods.get_mut(&id) {
            mod_info.enabled = false;
        }
        
        state.save().await?;
        println!("{} {} disabled.", "✔".green(), mod_name.cyan());
    } else {
        println!("{} Mod '{}' not found in installed list.", "⚠".yellow(), mod_name.magenta());
    }

    Ok(())
}
