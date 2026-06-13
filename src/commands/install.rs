use anyhow::Result;
use crate::core::downloader::Downloader;
use crate::core::state::{InstalledMod, KmgrState};
use std::path::Path;
use tokio::fs;
use colored::Colorize;

/// Installs a new package or dependency into the environment.
///
/// Takes the package identifier, an optional Minecraft version string, and an
/// optional provider source hint. Resolves the requested package, downloads the
/// required artifacts, cleans up obsolete versions, and registers the component
/// in local state.
pub async fn do_cmd(mod_name: String, mc_version: Option<String>, source_opt: Option<String>) -> Result<()> {
    let mut _lock = fslock::LockFile::open("kmgr.flock")?;
    _lock.lock()?;

    let mut state = KmgrState::load().await?;
    state.check_initialized()?;
    let version_str = mc_version.unwrap_or_else(|| state.default_mc_version.clone());
    
    let registry = crate::clients::build_registry()?;
    let provider = match &source_opt {
        Some(src) => registry.get(src)?,
        None => registry.get_default()?,
    };
    
    println!("{} Installing {} for Minecraft {} (Source: {})\n", "".cyan().bold(), mod_name.green(), version_str.yellow(), provider.display_name().magenta());
    
    if !Path::new(&state.mods_folder).exists() {
        fs::create_dir_all(&state.mods_folder).await?;
    }
    
    let downloader = Downloader::new();
    
    match provider.resolve(&mod_name, &version_str, &state.mod_loader).await {
        Ok(versions_to_install) => {
            if versions_to_install.is_empty() {
                println!("   {} No installable payload found.", "⚠".yellow());
                return Ok(());
            }

            println!("{} {} packages to install", "=>".cyan().bold(), versions_to_install.len().to_string().yellow());
            
            let mut is_first = true;
            for target in &versions_to_install {
                let is_explicit = is_first;
                is_first = false;

                if let Some(existing) = state.installed_mods.get(&target.id) {
                    if existing.version == target.version {
                        let change_to_explicit = is_explicit && !existing.is_explicit;
                        println!("    {} {} is already installed (v{}){}", 
                            "=".bright_black(), 
                            target.name.cyan(), 
                            target.version,
                            if change_to_explicit { " - marked as explicit".bright_black().to_string() } else { "".to_string() }
                        );
                        
                        if change_to_explicit {
                            let mut updated = existing.clone();
                            updated.is_explicit = true;
                            state.installed_mods.insert(target.id.clone(), updated);
                            
                            if let Some(profile) = state.profiles.get_mut(&state.active_profile) {
                                if !profile.contains(&target.id) {
                                    profile.push(target.id.clone());
                                }
                            }
                        }
                        continue;
                    }
                }

                let dest = state.get_mod_path(&target.filename, true);
                println!("    {} Downloading {}", "↓".blue(), target.filename.bright_black());
                
                if let Err(e) = downloader.download_file(&target.download_url, &dest, target.hash.as_deref()).await {
                    eprintln!("      {} Failed: {}", "✗".red(), e);
                } else {
                    println!("      {} Done", "✔".green());
                    
                    if let Some(existing) = state.installed_mods.get(&target.id) {
                        if existing.filename != target.filename {
                            let old_file_path = state.get_mod_path(&existing.filename, existing.enabled);
                            let _ = tokio::fs::remove_file(&old_file_path).await;
                        }
                    }

                    if is_explicit {
                        if let Some(profile) = state.profiles.get_mut(&state.active_profile) {
                            if !profile.contains(&target.id) {
                                profile.push(target.id.clone());
                            }
                        }
                    }

                    state.installed_mods.insert(
                        target.id.clone(),
                        InstalledMod {
                            name: target.name.clone(),
                            version: target.version.clone(),
                            source: target.source.clone(),
                            filename: target.filename.clone(),
                            download_url: target.download_url.clone(),
                            hash: target.hash.clone(),
                            is_explicit,
                            dependencies: target.dependencies.clone(),
                            enabled: true,
                        }
                    );
                }
            }
            
            state.save().await?;
            println!("\n{} Installation complete.", "".green().bold());
        }
        Err(e) => {
            eprintln!("{} Dependency resolution failed: {}", "Error:".red().bold(), e);
        }
    }

    Ok(())
}
