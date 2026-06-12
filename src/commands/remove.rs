use anyhow::Result;
use crate::core::state::KmgrState;
use colored::Colorize;
use tokio::fs;

/// Removes a package from the environment.
///
/// Takes the requested package identifier. It deletes the physical artifact
/// from the disk, unregisters it from all active profiles, and drops it from
/// the local state configuration.
pub async fn do_cmd(mod_name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;
    state.check_initialized()?;
    
    if let Some(id) = state.find_mod_id(&mod_name) {
        println!("{} Removing {}...", "::".cyan().bold(), mod_name.green());
        let (file, path) = {
            let mod_info = state.installed_mods.get(&id).unwrap();
            (mod_info.filename.clone(), state.get_mod_path(&mod_info.filename, mod_info.enabled))
        };
        
        if let Err(e) = fs::remove_file(&path).await {
            eprintln!("   {} Failed to delete {}: {}", "⚠".yellow(), file, e);
        } else {
            println!("   {} Deleted {}", "✔".green(), file.bright_black());
        }
        
        state.installed_mods.remove(&id);
        
        for profile_mods in state.profiles.values_mut() {
            profile_mods.retain(|x| x != &id);
        }

        state.save().await?;
        println!("{} Successfully uninstalled.", "::".green().bold());
        println!("   {} Tip: Run `kmgr prune` to remove unused dependencies.", "ℹ".blue());
    } else {
        println!("{} Mod '{}' not found in installed list.", "⚠".yellow(), mod_name.magenta());
    }

    Ok(())
}

