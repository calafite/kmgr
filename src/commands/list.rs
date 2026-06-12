use anyhow::Result;
use crate::core::state::KmgrState;
use colored::Colorize;

/// Prints the currently tracked packages.
///
/// Fetches the local application state and outputs a formatted list to stdout,
/// including current package states, versions, and downstream dependencies.
pub async fn do_cmd() -> Result<()> {
    let state = KmgrState::load().await?;
    state.check_initialized()?;
    
    let active = state.active_profile.clone();
    println!("{} Installed mods (Profile: {}, Minecraft {}, Loader: {}):\n", "::".cyan().bold(), active.green(), state.default_mc_version.yellow(), state.mod_loader.magenta());

    if state.installed_mods.is_empty() {
        println!("   No mods installed.");
        return Ok(());
    }

    let mut mods: Vec<_> = state.installed_mods.iter().collect();
    mods.sort_by_key(|(_, m)| &m.name);

    for (_id, mod_info) in mods {
        let version_fmt = format!("v{}", mod_info.version).green();
        let src_fmt = format!("[{}]", mod_info.source).bright_black();
        let type_fmt = if mod_info.is_explicit {
            "".to_string()
        } else {
            " (dependency)".bright_black().to_string()
        };
        let status_fmt = if mod_info.enabled {
            "".to_string()
        } else {
            " [DISABLED]".red().to_string()
        };
        let name_fmt = if mod_info.enabled {
            mod_info.name.cyan()
        } else {
            mod_info.name.bright_black()
        };

        println!("   - {} {}{}{}{}", name_fmt, version_fmt, src_fmt, type_fmt, status_fmt);
    }
    
    Ok(())
}
