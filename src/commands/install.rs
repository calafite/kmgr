use crate::core::downloader::Downloader;
use crate::core::state::{InstalledMod, KmgrState};
use anyhow::Result;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

struct InstallTaskResult {
    id: String,
    target: crate::core::provider::ResolvedTarget,
    is_explicit: bool,
    status: InstallStatus,
}

enum InstallStatus {
    AlreadyInstalled {
        change_to_explicit: bool,
    },
    Downloaded {
        installed_mod: InstalledMod,
        old_filename_to_remove: Option<String>,
    },
}

/// Installs a new package or dependency into the environment.
///
/// Takes the package identifier, an optional Minecraft version string, and an
/// optional provider source hint. Resolves the requested package, downloads the
/// required artifacts, cleans up obsolete versions, and registers the component
/// in local state.
pub async fn do_cmd(
    mod_name: String,
    mc_version: Option<String>,
    source_opt: Option<String>,
    jobs: usize,
) -> Result<()> {
    let (mut state, _lock) = KmgrState::lock_and_load().await?;

    state.check_initialized()?;
    let version_str = mc_version.unwrap_or_else(|| state.default_mc_version.clone());

    let registry = crate::clients::build_registry()?;
    let provider = match &source_opt {
        Some(src) => registry.get(src)?,
        None => registry.get_default()?,
    };

    println!(
        "{} Installing {} for Minecraft {} (Source: {})\n",
        "".cyan().bold(),
        mod_name.green(),
        version_str.yellow(),
        provider.display_name().magenta()
    );

    if !Path::new(&state.mods_folder).exists() {
        fs::create_dir_all(&state.mods_folder).await?;
    }

    let downloader = Arc::new(Downloader::new());

    match provider
        .resolve(&mod_name, &version_str, &state.mod_loader, jobs)
        .await
    {
        Ok(versions_to_install) => {
            if versions_to_install.is_empty() {
                println!("   {} No installable payload found.", "⚠".yellow());
                return Ok(());
            }

            println!(
                "{} {} packages to install",
                "=>".cyan().bold(),
                versions_to_install.len().to_string().yellow()
            );

            let concurrency_limit = jobs;
            let mods_folder = state.mods_folder.clone();

            let install_tasks: Vec<Result<InstallTaskResult>> =
                stream::iter(versions_to_install.into_iter().enumerate())
                    .map(|(i, target)| {
                        let is_explicit = i == 0;
                        let downloader = downloader.clone();
                        let existing_mod = state.installed_mods.get(&target.id).cloned();
                        let safe_filename = Path::new(&target.filename)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("unknown.jar");
                        let dest_path = Path::new(&mods_folder)
                            .join(safe_filename)
                            .to_string_lossy()
                            .to_string();

                        async move {
                            if let Some(ref existing) = existing_mod {
                                if existing.version == target.version {
                                    let change_to_explicit = is_explicit && !existing.is_explicit;
                                    return Ok(InstallTaskResult {
                                        id: target.id.clone(),
                                        target,
                                        is_explicit,
                                        status: InstallStatus::AlreadyInstalled {
                                            change_to_explicit,
                                        },
                                    });
                                }
                            }

                            downloader.println(&format!(
                                "    {} Downloading {}",
                                "↓".blue(),
                                target.filename.bright_black()
                            ));

                            downloader
                                .download_file(
                                    &target.download_url,
                                    &dest_path,
                                    target.hash.as_deref(),
                                )
                                .await?;

                            let old_filename_to_remove = if let Some(ref existing) = existing_mod {
                                if existing.filename != target.filename {
                                    Some(existing.filename.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            let installed_mod = InstalledMod {
                                name: target.name.clone(),
                                version: target.version.clone(),
                                source: target.source.clone(),
                                filename: target.filename.clone(),
                                download_url: target.download_url.clone(),
                                hash: target.hash.clone(),
                                is_explicit,
                                dependencies: target.dependencies.clone(),
                                enabled: true,
                            };

                            Ok(InstallTaskResult {
                                id: target.id.clone(),
                                target,
                                is_explicit,
                                status: InstallStatus::Downloaded {
                                    installed_mod,
                                    old_filename_to_remove,
                                },
                            })
                        }
                    })
                    .buffer_unordered(concurrency_limit)
                    .collect()
                    .await;

            let mut has_errors = false;

            for res in install_tasks {
                match res {
                    Ok(task_result) => match task_result.status {
                        InstallStatus::AlreadyInstalled { change_to_explicit } => {
                            println!(
                                "    {} {} is already installed (v{}){}",
                                "=".bright_black(),
                                task_result.target.name.cyan(),
                                task_result.target.version,
                                if change_to_explicit {
                                    " - marked as explicit".bright_black().to_string()
                                } else {
                                    "".to_string()
                                }
                            );

                            if change_to_explicit {
                                if let Some(existing) =
                                    state.installed_mods.get_mut(&task_result.id)
                                {
                                    existing.is_explicit = true;
                                }
                                if let Some(profile) = state.profiles.get_mut(&state.active_profile)
                                {
                                    if !profile.contains(&task_result.id) {
                                        profile.push(task_result.id.clone());
                                    }
                                }
                            }
                        }
                        InstallStatus::Downloaded {
                            installed_mod,
                            old_filename_to_remove,
                        } => {
                            println!("      {} Done", "✔".green());

                            if let Some(old_filename) = old_filename_to_remove {
                                let enabled = state
                                    .installed_mods
                                    .get(&task_result.id)
                                    .map(|m| m.enabled)
                                    .unwrap_or(true);
                                let old_file_path = state.get_mod_path(&old_filename, enabled);
                                let _ = tokio::fs::remove_file(&old_file_path).await;
                            }

                            if task_result.is_explicit {
                                if let Some(profile) = state.profiles.get_mut(&state.active_profile)
                                {
                                    if !profile.contains(&task_result.id) {
                                        profile.push(task_result.id.clone());
                                    }
                                }
                            }

                            state.installed_mods.insert(task_result.id, installed_mod);
                        }
                    },
                    Err(e) => {
                        eprintln!("      {} Failed: {}", "✗".red(), e);
                        has_errors = true;
                    }
                }
            }

            state.save().await?;

            if has_errors {
                println!(
                    "\n{} Installation completed with errors.",
                    "⚠".yellow().bold()
                );
            } else {
                println!("\n{} Installation complete.", "".green().bold());
            }
        }
        Err(e) => {
            eprintln!(
                "{} Dependency resolution failed: {}",
                "Error:".red().bold(),
                e
            );
        }
    }

    Ok(())
}
