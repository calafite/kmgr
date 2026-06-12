use crate::core::state::KmgrState;
use anyhow::Result;
use colored::Colorize;

/// Audits and applies package updates.
///
/// Takes a boolean flag indicating whether updates should be applied to disk.
/// Polls remote registries for newer versions of installed packages, reports
/// the differences, and replaces artifacts if requested.
pub async fn do_cmd(apply: bool) -> Result<()> {
    let mut state = KmgrState::load().await?;
    state.check_initialized()?;

    println!(
        "{} Checking installed dependencies...\n",
        "".cyan().bold()
    );

    if state.installed_mods.is_empty() {
        println!(
            "   No mods installed. Use `{} {}` to install mods.",
            "kmgr install".cyan(),
            "<mod_name>".yellow()
        );
        return Ok(());
    }

    let registry = std::sync::Arc::new(crate::clients::build_registry());
    let downloader = crate::core::downloader::Downloader::new();

    println!("  {}\n", "Currently installed mods:".bright_black());

    let mut updates_available = 0;
    let mut applied_count = 0;

    let installed_mods = state.installed_mods.clone();

    let mut resolve_futures = Vec::new();
    let default_mc_version = state.default_mc_version.clone();
    let mod_loader = state.mod_loader.clone();

    for (id, mod_info) in &installed_mods {
        let registry_clone = registry.clone();
        let id_clone = id.clone();
        let source = mod_info.source.clone();
        let mc_version = default_mc_version.clone();
        let loader = mod_loader.clone();

        resolve_futures.push(tokio::spawn(async move {
            let mut latest_version = None;
            let mut update_target = None;
            if let Ok(provider) = registry_clone.get(&source) {
                if let Ok(targets) = provider.resolve(&id_clone, &mc_version, &loader).await {
                    if let Some(target) = targets.into_iter().next() {
                        latest_version = Some(target.version.clone());
                        update_target = Some(target);
                    }
                }
            }
            (id_clone, latest_version, update_target)
        }));
    }

    let resolved_results = futures::future::join_all(resolve_futures).await;
    let mut resolutions = std::collections::HashMap::new();
    for res in resolved_results {
        if let Ok((id, latest_version, update_target)) = res {
            resolutions.insert(id, (latest_version, update_target));
        }
    }

    for (id, mod_info) in installed_mods {
        let version_fmt = format!("v{}", mod_info.version).green();
        let src_fmt = format!("[{}]", mod_info.source).bright_black();
        print!(
            "   - {} {} {}",
            mod_info.filename.cyan(),
            version_fmt,
            src_fmt
        );

        let (latest_version, update_target) = resolutions.remove(&id).unwrap_or((None, None));

        if let Some(latest) = latest_version {
            if latest != mod_info.version && latest != "latest" {
                println!(" {} {}", "→ v".yellow(), latest.yellow().bold());
                updates_available += 1;

                if apply {
                    if let Some(target) = update_target {
                        let dest = state.get_mod_path(&target.filename, mod_info.enabled);
                        println!(
                            "      {} Downloading {}...",
                            "↓".blue(),
                            target.filename.bright_black()
                        );
                        if let Err(e) = downloader
                            .download_file(&target.download_url, &dest, target.hash.as_deref())
                            .await
                        {
                            eprintln!("        {} Failed: {}", "✗".red(), e);
                        } else {
                            println!("        {} Done", "✔".green());

                            if target.filename != mod_info.filename {
                                let old_file_path =
                                    state.get_mod_path(&mod_info.filename, mod_info.enabled);
                                let _ = tokio::fs::remove_file(&old_file_path).await;
                            }

                            state.installed_mods.insert(
                                id.clone(),
                                crate::core::state::InstalledMod {
                                    name: mod_info.name.clone(),
                                    version: target.version.clone(),
                                    source: target.source.clone(),
                                    filename: target.filename.clone(),
                                    download_url: target.download_url.clone(),
                                    hash: target.hash.clone(),
                                    is_explicit: mod_info.is_explicit,
                                    dependencies: target.dependencies.clone(),
                                    enabled: mod_info.enabled,
                                },
                            );
                            applied_count += 1;
                        }
                    }
                }
            } else {
                println!(" {} {}", "→".bright_black(), "up to date".bright_black());
            }
        } else {
            println!(
                " {} {}",
                "→".bright_black(),
                "unknown status".bright_black()
            );
        }
    }

    if updates_available > 0 {
        if apply {
            state.save().await?;
            println!(
                "\n{} Successfully applied {} updates.",
                "".green().bold(),
                applied_count.to_string().yellow()
            );
        } else {
            println!(
                "\n{} {} updates ready to be applied (Run `kmgr update --apply` to install)",
                "=>".cyan().bold(),
                updates_available.to_string().yellow()
            );
        }
    } else {
        println!(
            "\n{} {}",
            "=>".cyan().bold(),
            "All systems up to date.".bright_black()
        );
    }

    Ok(())
}
