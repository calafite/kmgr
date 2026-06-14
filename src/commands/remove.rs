use crate::core::state::KmgrState;
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::io::{self, Write};
use tokio::fs;

pub async fn do_cmd(mod_name: String) -> Result<()> {
    let (mut state, _lock) = KmgrState::lock_and_load().await?;
    state.check_initialized()?;

    if let Some((id, matched_name)) = state.find_mod_id_fuzzy(&mod_name) {
        if matched_name.to_lowercase() != mod_name.to_lowercase() {
            println!("   {} Matched '{}'", "~".yellow(), matched_name.cyan());
        }

        print!(
            "   {} Are you sure you want to remove '{}'? [y/N]: ",
            "?".yellow(),
            matched_name.cyan()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let val = input.trim().to_lowercase();

        if val != "y" && val != "yes" {
            println!("   {} Aborted.", "✗".red());
            return Ok(());
        }

        println!("{} Removing {}...", "".cyan().bold(), matched_name.green());
        let (file, path) = {
            let mod_info = state.installed_mods.get(&id).ok_or_else(|| {
                anyhow!(
                    "Corrupted state: Mod ID '{}' not found in installed list",
                    id
                )
            })?;
            (
                mod_info.filename.clone(),
                state.get_mod_path(&mod_info.filename, mod_info.enabled),
            )
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
        println!("{} Successfully uninstalled.", "".green().bold());
        println!(
            "   {} Tip: Run `kmgr prune` to remove unused dependencies.",
            "ℹ".blue()
        );
    } else {
        println!(
            "{} No mod matching '{}' found.",
            "⚠".yellow(),
            mod_name.magenta()
        );
    }

    Ok(())
}
