use anyhow::Result;
use colored::Colorize;

/// Performs a registry query.
///
/// Takes a search string and an optional source registry identifier.
/// Retrieves matching packages from the remote provider and outputs them.
pub async fn do_cmd(query: String, source_opt: Option<String>) -> Result<()> {
    let registry = crate::clients::build_registry();
    let provider = match &source_opt {
        Some(src) => registry.get(src)?,
        None => registry.get_default()?,
    };

    println!("{} Searching {} for '{}'...\n", "".cyan().bold(), provider.display_name().magenta(), query.yellow());

    match provider.search(&query).await {
        Ok(results) => {
            if results.is_empty() {
                println!("   No results found.");
            } else {
                for hit in results {
                    let extra_label = hit.extra.map(|e| format!(" [{}]", e)).unwrap_or_default().bright_black();
                    println!("  {} {}{}", hit.title.cyan().bold(), hit.id_or_slug.bright_black(), extra_label);
                    println!("    {}", hit.description.white());
                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("{} Failed to search {}: {}", "Error:".red().bold(), provider.display_name(), e);
        }
    }

    Ok(())
}
