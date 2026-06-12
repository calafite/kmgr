use crate::clients::modrinth::{ModrinthClient, Version};
use anyhow::Result;
use colored::Colorize;
use std::collections::{HashMap, HashSet};

/// Resolves dependencies using a Breadth-First Search approach.
/// Currently optimized for Modrinth as it exposes a structural dependency graph.
pub struct DependencyResolver {
    modrinth: ModrinthClient,
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            modrinth: ModrinthClient::new(),
        }
    }

    /// Takes a root project slug/id and traverses the dependency tree.
    /// Returns a list of all required `Version` manifests to download.
    pub async fn resolve_modrinth(
        &self,
        project_slug: &str,
        mc_version: &str,
    ) -> Result<Vec<Version>> {
        let mut resolved_versions: HashMap<String, Version> = HashMap::new(); // project_id -> Version
        let mut queue: Vec<String> = vec![project_slug.to_string()];
        let mut seen_projects: HashSet<String> = HashSet::new();

        println!(
            "{} Resolving dependencies for {} on MC {}...",
            "::".cyan().bold(),
            project_slug.green(),
            mc_version.yellow()
        );

        while let Some(current_req) = queue.pop() {
            if seen_projects.contains(&current_req) {
                continue;
            }

            // 1. Resolve project info
            let project = match self.modrinth.get_project(&current_req).await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to fetch project info for {}: {}", current_req, e);
                    continue;
                }
            };

            seen_projects.insert(project.id.clone());
            seen_projects.insert(project.slug.clone());

            // 2. Resolve target version for MC version
            let mut versions = self.modrinth.get_versions(&project.id, mc_version).await?;

            if let Some(target_version) = versions.pop() {
                // Get latest compatible version
                let v_str = format!("v{}", target_version.version_number).bright_black();
                println!("   {} {} {}", "✔".green(), project.title.cyan(), v_str);

                // 3. Process dependencies
                for dep in &target_version.dependencies {
                    if dep.dependency_type == "required" {
                        if let Some(dep_proj_id) = &dep.project_id {
                            if !resolved_versions.contains_key(dep_proj_id) {
                                queue.push(dep_proj_id.clone());
                            }
                        } else if let Some(dep_version_id) = &dep.version_id {
                            // Rare edge case: Modrinth hard-links a specific version but no project_id
                            if let Ok(v) = self.modrinth.get_version(dep_version_id).await {
                                queue.push(v.project_id.clone());
                            }
                        }
                    }
                }

                resolved_versions.insert(project.id.clone(), target_version);
            } else {
                eprintln!(
                    "   {} No compatible version found for '{}' on MC {}",
                    "⚠".yellow(),
                    project.title.magenta(),
                    mc_version
                );
            }
        }

        Ok(resolved_versions.into_values().collect())
    }
}
