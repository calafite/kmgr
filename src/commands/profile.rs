use crate::cli::ProfileCommands;
use crate::core::state::KmgrState;
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::collections::HashSet;
use tokio::fs;

/// Dispatches profile-related subcommands.
///
/// Takes a ProfileCommands enum variant and routes it to the correct handler.
pub async fn do_cmd(command: ProfileCommands) -> Result<()> {
    let (mut state, _lock) = KmgrState::lock_and_load().await?;
    state.check_initialized()?;

    match command {
        ProfileCommands::List => list().await,
        ProfileCommands::Create { name } => create(name).await,
        ProfileCommands::Switch { name } => switch(name).await,
        ProfileCommands::Add { mods } => {
            for mod_name in mods {
                add(mod_name).await?;
            }
            Ok(())
        }
        ProfileCommands::Remove { mods } => {
            for mod_name in mods {
                remove(mod_name).await?;
            }
            Ok(())
        }
        ProfileCommands::Delete { name } => delete(name).await,
        ProfileCommands::Rename { old_name, new_name } => rename(old_name, new_name).await,
    }
}

/// Retrieves and renders all established layout profiles.
async fn list() -> Result<()> {
    let state = KmgrState::load().await?;
    println!("{} Profiles:", "".cyan().bold());

    for (name, mods) in &state.profiles {
        let active_mark = if name == &state.active_profile {
            "*".green().bold()
        } else {
            " ".normal()
        };
        println!(" {} {} ({} mods)", active_mark, name.cyan(), mods.len());
    }

    Ok(())
}

/// Sets up an empty application layout profile.
///
/// Takes the new profile name as a parameter.
async fn create(name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;

    if state.profiles.contains_key(&name) {
        println!("{} Profile '{}' already exists.", "⚠".yellow(), name.cyan());
        return Ok(());
    }

    state.profiles.insert(name.clone(), Vec::new());
    state.save().await?;

    println!(
        "{} Created profile '{}'. Use `kmgr profile switch {}` to switch.",
        "✔".green(),
        name.cyan(),
        name
    );
    Ok(())
}

/// Registers an existing package identifier with the active layout profile.
///
/// Takes the requested package name.
async fn add(mod_name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;

    if let Some(id) = state.find_mod_id(&mod_name) {
        let active = state.active_profile.clone();
        if let Some(profile) = state.profiles.get_mut(&active) {
            if !profile.contains(&id) {
                profile.push(id.clone());
                state.save().await?;
                println!(
                    "{} Added '{}' to profile '{}'.",
                    "✔".green(),
                    mod_name.cyan(),
                    active
                );
            } else {
                println!(
                    "{} '{}' is already in profile '{}'.",
                    "=".bright_black(),
                    mod_name.cyan(),
                    active
                );
            }
        }
    } else {
        println!(
            "{} Mod '{}' not found in installed list.",
            "⚠".yellow(),
            mod_name.magenta()
        );
    }
    Ok(())
}

/// Detaches a package identifier from the active layout profile.
///
/// Takes the selected package name.
async fn remove(mod_name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;

    if let Some(id) = state.find_mod_id(&mod_name) {
        let active = state.active_profile.clone();
        if let Some(profile) = state.profiles.get_mut(&active) {
            if profile.contains(&id) {
                profile.retain(|x| x != &id);
                state.save().await?;
                println!(
                    "{} Removed '{}' from profile '{}'.",
                    "✔".green(),
                    mod_name.cyan(),
                    active
                );
                println!(
                    "   {} Tip: Run `kmgr profile switch {}` to apply changes.",
                    "ℹ".blue(),
                    active
                );
            } else {
                println!(
                    "{} '{}' is not in profile '{}'.",
                    "⚠".yellow(),
                    mod_name.cyan(),
                    active
                );
            }
        }
    } else {
        println!(
            "{} Mod '{}' not found in installed list.",
            "⚠".yellow(),
            mod_name.magenta()
        );
    }
    Ok(())
}

/// Swaps the environmental workload to the designated profile.
///
/// Takes the name of the target profile. Computes the union of explicit
/// dependencies required for the profile, deactivates disjoint components,
/// and re-activates required artifacts.
async fn switch(name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;

    if !state.profiles.contains_key(&name) {
        eprintln!(
            "{} Profile '{}' does not exist.",
            "Error:".red().bold(),
            name.yellow()
        );
        return Err(anyhow!("Profile not found"));
    }

    let mut reachable = HashSet::new();
    let mut queue = Vec::new();

    if let Some(profile_mods) = state.profiles.get(&name) {
        for m in profile_mods {
            reachable.insert(m.clone());
            queue.push(m.clone());
        }
    }

    while let Some(current_id) = queue.pop() {
        if let Some(mod_info) = state.installed_mods.get(&current_id) {
            for dep_id in &mod_info.dependencies {
                if !reachable.contains(dep_id) {
                    reachable.insert(dep_id.clone());
                    queue.push(dep_id.clone());
                }
            }
        }
    }

    let mut to_enable = Vec::new();
    let mut to_disable = Vec::new();

    for (id, mod_info) in &state.installed_mods {
        if reachable.contains(id) && !mod_info.enabled {
            to_enable.push(id.clone());
        } else if !reachable.contains(id) && mod_info.enabled {
            to_disable.push(id.clone());
        }
    }

    println!(
        "{} Switching to profile '{}'...",
        "".cyan().bold(),
        name.green()
    );

    if to_enable.is_empty() && to_disable.is_empty() {
        println!("   {} No changes needed.", "=".bright_black());
    } else {
        for id in to_enable {
            let filename = match state.installed_mods.get(&id) {
                Some(m) => m.filename.clone(),
                None => continue,
            };
            let disabled_path = state.get_mod_path(&filename, false);
            let enabled_path = state.get_mod_path(&filename, true);

            if let Some(info) = state.installed_mods.get_mut(&id) {
                if tokio::fs::try_exists(&disabled_path).await? {
                    if let Err(e) = fs::rename(&disabled_path, &enabled_path).await {
                        eprintln!("   {} Failed to enable {}: {}", "✗".red(), info.name, e);
                    } else {
                        println!("   {} Enabled {}", "+".green(), info.name);
                        info.enabled = true;
                    }
                } else {
                    info.enabled = true;
                }
            }
        }

        for id in to_disable {
            let filename = match state.installed_mods.get(&id) {
                Some(m) => m.filename.clone(),
                None => continue,
            };
            let enabled_path = state.get_mod_path(&filename, true);
            let disabled_path = state.get_mod_path(&filename, false);

            if let Some(info) = state.installed_mods.get_mut(&id) {
                if tokio::fs::try_exists(&enabled_path).await? {
                    if let Err(e) = fs::rename(&enabled_path, &disabled_path).await {
                        eprintln!("   {} Failed to disable {}: {}", "✗".red(), info.name, e);
                    } else {
                        println!("   {} Disabled {}", "-".red(), info.name);
                        info.enabled = false;
                    }
                } else {
                    info.enabled = false;
                }
            }
        }
    }

    state.active_profile = name.clone();
    state.save().await?;
    println!("{} Successfully switched profile.", "✔".green());

    Ok(())
}

/// Erases a layout profile permanently.
///
/// Takes the target profile name string.
async fn delete(name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;

    if name == "default" {
        println!("{} Cannot delete the default profile.", "⚠".yellow());
        return Ok(());
    }

    if !state.profiles.contains_key(&name) {
        println!("{} Profile '{}' not found.", "⚠".yellow(), name.cyan());
        return Ok(());
    }

    if state.active_profile == name {
        println!(
            "{} Cannot delete the currently active profile.",
            "⚠".yellow()
        );
        println!(
            "   {} Tip: Run `kmgr profile switch default` first.",
            "ℹ".blue()
        );
        return Ok(());
    }

    state.profiles.remove(&name);
    state.save().await?;

    println!("{} Deleted profile '{}'.", "✔".green(), name.cyan());
    Ok(())
}

/// Relabels a layout profile index key.
///
/// Takes the current string identifier and the replacement target identifier.
async fn rename(old_name: String, new_name: String) -> Result<()> {
    let mut state = KmgrState::load().await?;

    if old_name == "default" {
        println!("{} Cannot rename the default profile.", "⚠".yellow());
        return Ok(());
    }

    if !state.profiles.contains_key(&old_name) {
        println!("{} Profile '{}' not found.", "⚠".yellow(), old_name.cyan());
        return Ok(());
    }

    if state.profiles.contains_key(&new_name) {
        println!(
            "{} Profile '{}' already exists.",
            "⚠".yellow(),
            new_name.cyan()
        );
        return Ok(());
    }

    let mods = state
        .profiles
        .remove(&old_name)
        .ok_or_else(|| anyhow!("Profile '{}' not found", old_name))?;
    state.profiles.insert(new_name.clone(), mods);

    if state.active_profile == old_name {
        state.active_profile = new_name.clone();
    }

    state.save().await?;
    println!(
        "{} Renamed profile '{}' to '{}'.",
        "✔".green(),
        old_name.cyan(),
        new_name.cyan()
    );

    Ok(())
}
