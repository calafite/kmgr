use crate::core::state::KmgrState;
use anyhow::{Result, anyhow};
use colored::Colorize;
use tokio::fs;

pub async fn do_cmd(mod_name: String) -> Result<()> {
    let (mut state, _lock) = KmgrState::lock_and_load().await?;
    state.check_initialized()?;

    if let Some((id, matched_name)) = state.find_mod_id_fuzzy(&mod_name) {
        if matched_name.to_lowercase() != mod_name.to_lowercase() {
            println!("   {} Matched '{}'", "~".yellow(), matched_name.cyan());
        }

        let (current_filename, was_enabled) = {
            let mod_info = state.installed_mods.get(&id).ok_or_else(|| {
                anyhow!(
                    "Corrupted state: Mod ID '{}' not found in installed list",
                    id
                )
            })?;
            (mod_info.filename.clone(), mod_info.enabled)
        };

        if was_enabled {
            println!(
                "{} Mod '{}' is already enabled.",
                "".cyan().bold(),
                mod_name.green()
            );
            return Ok(());
        }

        let disabled_path = state.get_mod_path(&current_filename, false);
        let enabled_path = state.get_mod_path(&current_filename, true);

        match fs::rename(&disabled_path, &enabled_path).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "{} Warning: {} was not found, but marking as enabled in config.",
                    "⚠".yellow(),
                    disabled_path.bright_black()
                );
            }
            Err(e) => {
                eprintln!(
                    "{} Failed to enable mod (rename failed): {}",
                    "Error:".red().bold(),
                    e
                );
                return Err(anyhow!("Rename failed"));
            }
        }

        if let Some(mod_info) = state.installed_mods.get_mut(&id) {
            mod_info.enabled = true;
        }

        state.save().await?;
        println!("{} {} enabled.", "✔".green(), mod_name.cyan());
    } else {
        println!(
            "{} No mod matching '{}' found.",
            "⚠".yellow(),
            mod_name.magenta()
        );
    }

    Ok(())
}
