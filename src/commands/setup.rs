use crate::core::state::KmgrState;
use anyhow::Result;
use colored::Colorize;
use std::io::{self, Write};
use tokio::fs;

/// Interactively builds the environment configuration with robust validation and normalization.
pub async fn do_cmd() -> Result<()> {
    println!("{} Starting interactive setup...", "::".cyan().bold());

    let mut state = KmgrState::load().await?;

    // 1. Prompt and Validate Minecraft Version
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

        // Normalize: lowercase, trim spaces
        let normalized = val.to_lowercase();

        // Validate version format: alphanumeric, dots, dashes, underscores only
        // Must contain at least one digit
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

    // 2. Prompt and Validate Mod Loader
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

        // Normalize: lowercase, trim spaces
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

    // 3. Prompt and Validate Mods Folder Path
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

        // Validate path: can't contain bad characters
        let invalid_chars = ['\0', '*', '?', '"', '<', '>', '|'];
        if val.chars().any(|c| invalid_chars.contains(&c)) {
            eprintln!(
                "      {} Path contains invalid characters. Please avoid using '*', '?', or quotes in path names.",
                "✗".red()
            );
            continue;
        }

        // Normalize path: trim trailing slashes, replace windows-style slashes if they entered them
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

    // Update state fields
    state.default_mc_version = mc_version;
    state.mod_loader = mod_loader;
    state.mods_folder = mods_folder;

    // Verify directory creation/existence
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
