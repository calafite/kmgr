use crate::core::state::KmgrState;
use anyhow::Result;
use colored::Colorize;
use std::io::{self, Write};
use tokio::fs;

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};

struct PathHelper(FilenameCompleter);

impl Completer for PathHelper {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.0.complete(line, pos, ctx)
    }
}

impl Hinter for PathHelper { type Hint = String; }
impl Highlighter for PathHelper {}
impl Validator for PathHelper {}
impl Helper for PathHelper {}

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
    let mut rl = rustyline::Editor::new()?;
    rl.set_helper(Some(PathHelper(FilenameCompleter::new())));

    loop {
        let has_current = !state.mods_folder.is_empty();
        let prompt_str = if has_current {
            format!(
                "   {} Mods folder (Type path or drag-and-drop) [current: {}]: ",
                "?".yellow(),
                state.mods_folder
            )
        } else {
            format!(
                "   {} Mods folder (Type path or drag-and-drop) [default: mods]: ",
                "?".yellow()
            )
        };

        let mut val = match rl.readline(&prompt_str) {
            Ok(line) => line.trim().to_string(),
            Err(rustyline::error::ReadlineError::Interrupted) | Err(rustyline::error::ReadlineError::Eof) => {
                println!("      {} Aborted.", "⚠".yellow());
                std::process::exit(1);
            }
            Err(err) => {
                eprintln!("      {} Readline error: {}", "✗".red(), err);
                continue;
            }
        };

        if val.is_empty() {
            if has_current {
                mods_folder = state.mods_folder.clone();
            } else {
                mods_folder = "mods".to_string();
            }
            break;
        }

        if (val.starts_with('\'') && val.ends_with('\''))
            || (val.starts_with('"') && val.ends_with('"'))
        {
            val = val[1..val.len() - 1].to_string();
        }

        if val.starts_with("~/") {
            if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
                val = val.replacen('~', &home, 1);
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
