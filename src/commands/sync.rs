use crate::core::downloader::Downloader;
use crate::core::state::KmgrState;
use crate::core::utils::*;
use anyhow::Result;
use colored::Colorize;
use std::path::Path;

/// Verifies disk artifacts against the application state configuration.
///
/// Iterates through declared deployment packages and checks their physical
/// presence on the system. If required files are missing, initiates remote
/// calls to synchronize the disk with the state declaration.
pub async fn do_cmd() -> Result<()> {
    let state = KmgrState::load().await?;
    state.check_initialized()?;
    let downloader = Downloader::new();

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

    let mut restored_count = 0;

    for (_, mod_info) in &state.installed_mods {
        let dest = state.get_mod_path(&mod_info.filename, mod_info.enabled);

        let mut needs_download = true;
        if Path::new(&dest).exists() {
            needs_download = false;

            if let Some(expected_hash) = &mod_info.hash {
                let is_sha1 = expected_hash.len() == 40;
                match compute_file_hash(&dest, is_sha1).await {
                    Ok(actual_hex) if &actual_hex != expected_hash => {
                        println!(
                            "   {} Checksum mismatch for '{}', re-downloading...",
                            "⚠".yellow(),
                            mod_info.filename
                        );
                        needs_download = true;
                    }
                    Err(e) => {
                        println!(
                            "   {} Failed to read '{}': {}",
                            "⚠".yellow(),
                            mod_info.filename,
                            e
                        );
                        needs_download = true;
                    }
                    _ => {}
                }
            }
        }

        if needs_download {
            if mod_info.download_url.is_empty() {
                eprintln!(
                    "   {} Cannot restore '{}' automatically (missing download URL in state). Try running `kmgr update --apply`.",
                    "⚠".yellow(),
                    mod_info.filename
                );
                continue;
            }

            println!(
                "   {} Restoring {}...",
                "↓".blue(),
                mod_info.filename.cyan()
            );
            if let Err(e) = downloader
                .download_file(&mod_info.download_url, &dest, mod_info.hash.as_deref())
                .await
            {
                eprintln!("      {} Failed: {}", "✗".red(), e);
                let _ = tokio::fs::remove_file(&dest).await;
            } else {
                println!("      {} Done", "✔".green());
                restored_count += 1;
            }
        }
    }

    if restored_count > 0 {
        println!(
            "\n{} Restored {} mod files.",
            "".green().bold(),
            restored_count.to_string().yellow()
        );
    } else {
        println!("{} All files are present and synced.", "✔".green());
    }

    Ok(())
}
