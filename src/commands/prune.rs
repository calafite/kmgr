use crate::core::state::KmgrState;
use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use tokio::fs;

/// Analyzes installed packages and purges disconnected dependencies.
///
/// Sweeps through all configured profiles, calculates the full tree of
/// required packages using local registry constraints, and deletes any
/// remaining artifacts that no longer have a parent dependency trace.
pub async fn do_cmd() -> Result<()> {
    let (mut state, _lock) = KmgrState::lock_and_load().await?;
    state.check_initialized()?;

    let mut reachable = HashSet::new();
    let mut queue = Vec::new();

    for profile_mods in state.profiles.values() {
        for id in profile_mods {
            if !reachable.contains(id) {
                reachable.insert(id.clone());
                queue.push(id.clone());
            }
        }
    }

    for (id, mod_info) in &state.installed_mods {
        if mod_info.is_explicit && !reachable.contains(id) {
            reachable.insert(id.clone());
            queue.push(id.clone());
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

    let mut to_remove = Vec::new();
    for (id, mod_info) in &state.installed_mods {
        if !reachable.contains(id) {
            let target_path = state.get_mod_path(&mod_info.filename, mod_info.enabled);
            to_remove.push((id.clone(), target_path, mod_info.name.clone()));
        }
    }

    if to_remove.is_empty() {
        println!("{} No orphaned dependencies to remove.", "".cyan().bold());
        return Ok(());
    }

    println!(
        "{} Removing {} orphaned dependencies...\n",
        "".cyan().bold(),
        to_remove.len().to_string().yellow()
    );

    let mut removed_count = 0;
    for (id, path, name) in to_remove {
        let _ = fs::remove_file(&path).await;
        println!("   {} Removed {}", "✔".green(), name.bright_black());
        state.installed_mods.remove(&id);
        removed_count += 1;
    }

    if removed_count > 0 {
        state.save().await?;
        println!(
            "\n{} Successfully pruned {} dependencies.",
            "=>".green().bold(),
            removed_count
        );
    }

    Ok(())
}
