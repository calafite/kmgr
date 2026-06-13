use crate::core::state::KmgrState;
use anyhow::Result;
use colored::Colorize;
use std::io::{self, Write};
use tokio::fs;

/// Interactively builds the environment configuration with robust validation and normalization.
/// Uses absolute paths, cannot resolve shell symbols (e.g, *)
pub async fn do_cmd() -> Result<()> {
    println!("{} Starting interactive setup...", "".cyan().bold());

    let (mut state, _lock) = KmgrState::lock_and_load().await?;

    let mc_version;
    loop {
        let has_current = !state.default_mc_version.is_empty();
        if has_current {
            print!(
                "   {} Minecraft version [current: {}]: ",
                "?".yellow(),
                state.default_mc_version
            );
        } else {
            print!("   {} Minecraft version (e.g. 1.20.4): ", "?".yellow());
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let val = input.trim();

        if val.is_empty() {
            if has_current {
                mc_version = state.default_mc_version.clone();
                break;
            } else {
                eprintln!(
                    "      {} Version cannot be empty. Please enter a Minecraft version.",
                    "✗".red()
                );
                continue;
            }
        }

        let normalized = val.to_lowercase();

        let has_digit = normalized.chars().any(|c| c.is_ascii_digit());
        let only_valid_chars = normalized
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_');

        if only_valid_chars && has_digit {
            mc_version = normalized;
            break;
        } else {
            eprintln!(
                "      {} Invalid Minecraft version format ({}). Standard format is major.minor[.patch] (e.g., 1.20.1).",
                "✗".red(),
                val.bold()
            );
        }
    }

    let mod_loader;
    let supported_loaders = ["fabric", "forge", "neoforge", "quilt"];
    loop {
        let has_current = !state.mod_loader.is_empty();
        if has_current {
            print!(
                "   {} Mod loader (fabric, forge, neoforge, quilt) [current: {}]: ",
                "?".yellow(),
                state.mod_loader
            );
        } else {
            print!(
                "   {} Mod loader (fabric, forge, neoforge, quilt): ",
                "?".yellow()
            );
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let val = input.trim();

        if val.is_empty() {
            if has_current {
                mod_loader = state.mod_loader.clone();
                break;
            } else {
                eprintln!(
                    "      {} Mod loader cannot be empty. Supported loaders: {}.",
                    "✗".red(),
                    supported_loaders.join(", ")
                );
                continue;
            }
        }

        let normalized = val.to_lowercase();

        if supported_loaders.contains(&normalized.as_str()) {
            mod_loader = normalized;
            break;
        } else {
            eprintln!(
                "      {} Invalid mod loader '{}'. Supported: {}.",
                "✗".red(),
                val.bold(),
                supported_loaders.join(", ")
            );
        }
    }

    let mods_folder;
    loop {
        let has_current = !state.mods_folder.is_empty();
        if has_current {
            print!(
                "   {} Mods folder path [current: {}]: ",
                "?".yellow(),
                state.mods_folder
            );
        } else {
            print!("   {} Mods folder path [default: mods]: ", "?".yellow());
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let val = input.trim();

        if val.is_empty() {
            if has_current {
                mods_folder = state.mods_folder.clone();
                break;
            } else {
                mods_folder = "mods".to_string();
                break;
            }
        }

        let invalid_chars = ['\0', '*', '?', '"', '<', '>', '|'];
        if val.chars().any(|c| invalid_chars.contains(&c)) {
            eprintln!(
                "      {} Path contains invalid characters. Please avoid using '*', '?', or quotes in path names.",
                "✗".red()
            );
            continue;
        }

        let mut normalized = val.replace('\\', "/");
        while normalized.ends_with('/') {
            normalized.pop();
        }

        if normalized.is_empty() {
            eprintln!("      {} Mods folder path cannot be empty.", "✗".red());
            continue;
        }

        mods_folder = normalized;
        break;
    }

    state.default_mc_version = mc_version;
    state.mod_loader = mod_loader;
    state.mods_folder = mods_folder;

    fs::create_dir_all(&state.mods_folder).await?;
    println!(
        "   {} Verified `{}` directory",
        "✔".green(),
        state.mods_folder
    );

    state.save().await?;

    println!(
        "\n{} {}",
        "=>".cyan().bold(),
        "Setup complete and saved!".green().bold()
    );
    println!(
        "   {} Minecraft: {}",
        "•".cyan(),
        state.default_mc_version.yellow()
    );
    println!(
        "   {} Mod Loader: {}",
        "•".cyan(),
        state.mod_loader.magenta()
    );
    println!(
        "   {} Mods Folder: {}",
        "•".cyan(),
        state.mods_folder.blue()
    );

    Ok(())
}
