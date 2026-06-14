use crate::core::downloader::Downloader;
use crate::core::state::KmgrState;
use crate::core::utils::*;
use anyhow::Result;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use std::path::Path;
use std::sync::Arc;

/// Verifies disk artifacts against the application state configuration.
///
/// Iterates through declared deployment packages and checks their physical
/// presence on the system. If required files are missing, initiates remote
/// calls to synchronize the disk with the state declaration.
pub async fn do_cmd(jobs: usize) -> Result<()> {
    let state = KmgrState::load().await?;
    state.check_initialized()?;

    println!("{} Syncing mod files from configs...\n", "".cyan().bold());

    if state.installed_mods.is_empty() {
        println!(
            "   {} No mods installed in configuration.",
            "=".bright_black()
        );
        return Ok(());
    }

    if !Path::new(&state.mods_folder).exists() {
        tokio::fs::create_dir_all(&state.mods_folder).await?;
    }

    let downloader = Arc::new(Downloader::new());
    let concurrency_limit = jobs;

    let installed_mods_vec: Vec<_> = state.installed_mods.clone().into_iter().collect();

    let sync_tasks = stream::iter(installed_mods_vec)
        .map(|(id, mod_info)| {
            let downloader_ref = downloader.clone();
            let dest = state.get_mod_path(&mod_info.filename, mod_info.enabled);
            let id_clone = id.clone();
            let mod_info_clone = mod_info.clone();

            async move {
                let mut needs_download = true;
                let mut download_success = false;
                let mut error_msg = None;

                if Path::new(&dest).exists() {
                    needs_download = false;
                    if let Some(expected_hash) = &mod_info_clone.hash {
                        let is_sha1 = expected_hash.len() == 40;
                        match compute_file_hash(&dest, is_sha1).await {
                            Ok(actual_hex) if &actual_hex != expected_hash => {
                                println!(
                                    "   {} Checksum mismatch for '{}', re-downloading...",
                                    "⚠".yellow(),
                                    mod_info_clone.filename
                                );
                                needs_download = true;
                            }
                            Err(e) => {
                                println!(
                                    "   {} Failed to compute checksum for '{}': {}. Re-downloading...",
                                    "⚠".yellow(),
                                    mod_info_clone.filename,
                                    e
                                );
                                needs_download = true;
                            }
                            _ => {} // Hashes match
                        }
                    }
                }

                if needs_download {
                    if mod_info_clone.download_url.is_empty() {
                        let msg = format!(
                            "Cannot restore '{}' automatically (missing download URL in state). Try running `kmgr update --apply`.",
                            mod_info_clone.filename
                        );
                        eprintln!("   {} {}", "⚠".yellow(), msg);
                        error_msg = Some(msg);
                    } else {
                        downloader_ref.println(&format!(
                            "   {} Restoring {}...",
                            "↓".blue(),
                            mod_info_clone.filename.cyan()
                        ));
                        if let Err(e) = downloader_ref
                            .download_file(&mod_info_clone.download_url, &dest, mod_info_clone.hash.as_deref())
                            .await
                        {
                            eprintln!("      {} Failed: {}", "✗".red(), e);
                            error_msg = Some(format!("Download failed: {}", e));
                            let _ = tokio::fs::remove_file(&dest).await;
                        } else {
                            println!("      {} Done", "✔".green());
                            download_success = true;
                        }
                    }
                }
                // Return status for this mod
                (id_clone, mod_info_clone, download_success, error_msg)
            }
        })
        .buffer_unordered(concurrency_limit);

    let mut restored_count = 0;
    let mut sync_errors = Vec::new();

    // Collect results
    let results: Vec<_> = sync_tasks.collect().await;
    for (_id, mod_info, download_success, error_msg) in results {
        if download_success {
            restored_count += 1;
        } else if let Some(msg) = error_msg {
            sync_errors.push(format!("{} ({})", mod_info.name, msg));
        }
    }

    if restored_count > 0 {
        println!(
            "\n{} Restored {} mod files.",
            "".green().bold(),
            restored_count.to_string().yellow()
        );
    } else if sync_errors.is_empty() {
        println!("{} All files are present and synced.", "✔".green());
    }

    if !sync_errors.is_empty() {
        eprintln!("\n{} Some mods failed to sync:", "✗".red().bold());
        for err in sync_errors {
            eprintln!("   - {}", err.red());
        }
    }

    Ok(())
}
