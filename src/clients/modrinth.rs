#![allow(dead_code)]

use anyhow::Result;
use reqwest::{header, Client};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use async_trait::async_trait;
use crate::core::provider::{ModProvider, ProviderSearchResult, ResolvedTarget};

pub struct ModrinthClient {
    client: Client,
    base_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SearchResponse {
    pub hits: Vec<SearchResult>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub project_type: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VersionFile {
    pub hashes: std::collections::HashMap<String, String>,
    pub url: String,
    pub filename: String,
    pub primary: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub dependencies: Vec<Dependency>,
    pub files: Vec<VersionFile>,
}

impl ModrinthClient {
    /// Creates a new instance of the Modrinth API client.
    pub fn new() -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("kmgr/0.1.0"),
        );

        ModrinthClient {
            client: Client::builder().default_headers(headers).build().unwrap(),
            base_url: "https://api.modrinth.com/v2".to_string(),
        }
    }

    /// Performs an internal search query against the Modrinth API.
    pub async fn search_mods_internal(&self, query: &str) -> Result<SearchResponse> {
        let url = format!("{}/search?query={}", self.base_url, query);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        Ok(response.json().await?)
    }

    /// Retrieves project details from Modrinth using its ID or slug.
    pub async fn get_project(&self, id_or_slug: &str) -> Result<Project> {
        let url = format!("{}/project/{}", self.base_url, id_or_slug);
        let response = self.client.get(&url).send().await?;
        
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("Project '{}' not found on Modrinth", id_or_slug);
        }
        
        Ok(response.error_for_status()?.json().await?)
    }

    /// Retrieves compatible versions of a project filtered by Minecraft version and loader.
    pub async fn get_versions(&self, project_id: &str, mc_version: &str, loader: &str) -> Result<Vec<Version>> {
        let game_versions = format!("[\"{}\"]", mc_version);
        let loaders = format!("[\"{}\"]", loader);
        let url = format!("{}/project/{}/version", self.base_url, project_id);
        
        let response = self.client.get(&url)
            .query(&[("game_versions", &game_versions), ("loaders", &loaders)])
            .send()
            .await?
            .error_for_status()?;
            
        Ok(response.json().await?)
    }
    
    /// Retrieves details of a specific version by its ID.
    pub async fn get_version(&self, version_id: &str) -> Result<Version> {
        let url = format!("{}/version/{}", self.base_url, version_id);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        Ok(response.json().await?)
    }
}

#[async_trait]
impl ModProvider for ModrinthClient {
    /// Returns the unique identifier for the Modrinth provider.
    fn id(&self) -> &'static str { "modrinth" }

    /// Returns the display name of the Modrinth provider.
    fn display_name(&self) -> &'static str { "Modrinth" }
    
    /// Searches for mods on Modrinth matching the query.
    async fn search(&self, query: &str) -> Result<Vec<ProviderSearchResult>> {
        let response = self.search_mods_internal(query).await?;
        
        let mapped = response.hits.into_iter().map(|h| ProviderSearchResult {
            title: h.title,
            description: h.description,
            id_or_slug: h.slug,
            extra: Some(format!("Project Type: {}", h.project_type)),
        }).collect();
        
        Ok(mapped)
    }

    /// Resolves a project and its required dependencies recursively.
    async fn resolve(&self, project_slug: &str, mc_version: &str, loader: &str) -> Result<Vec<ResolvedTarget>> {
        let mut resolved_versions: HashMap<String, Version> = HashMap::new();
        let mut queue: Vec<String> = vec![project_slug.to_string()];
        let mut seen_projects: HashSet<String> = HashSet::new();
        let mut project_names: HashMap<String, String> = HashMap::new();
        let mut project_deps: HashMap<String, Vec<String>> = HashMap::new();
        
        let mut root_project_id = None;

        while let Some(current_req) = queue.pop() {
            if seen_projects.contains(&current_req) {
                continue;
            }

            let project = match self.get_project(&current_req).await {
                Ok(p) => p,
                Err(_e) => {
                    continue;
                }
            };
            
            if root_project_id.is_none() {
                root_project_id = Some(project.id.clone());
            }
            
            seen_projects.insert(project.id.clone());
            seen_projects.insert(project.slug.clone());
            project_names.insert(project.id.clone(), project.title.clone());

            let mut versions = self.get_versions(&project.id, mc_version, loader).await?;
            
            if let Some(target_version) = versions.pop() {
                let mut deps_list = Vec::new();
                for dep in &target_version.dependencies {
                    if dep.dependency_type == "required" {
                        if let Some(dep_proj_id) = &dep.project_id {
                            if !resolved_versions.contains_key(dep_proj_id) {
                                queue.push(dep_proj_id.clone());
                            }
                            deps_list.push(dep_proj_id.clone());
                        } else if let Some(dep_version_id) = &dep.version_id {
                            if let Ok(v) = self.get_version(dep_version_id).await {
                                queue.push(v.project_id.clone());
                                deps_list.push(v.project_id.clone());
                            }
                        }
                    }
                }
                
                project_deps.insert(project.id.clone(), deps_list);
                resolved_versions.insert(project.id.clone(), target_version);
            }
        }

        let mut targets: Vec<ResolvedTarget> = resolved_versions.into_values().filter_map(|v| {
            let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first());
            file.map(|f| ResolvedTarget {
                id: v.project_id.clone(),
                name: project_names.get(&v.project_id).cloned().unwrap_or_else(|| v.project_id.clone()),
                download_url: f.url.clone(),
                hash: f.hashes.get("sha512").or_else(|| f.hashes.get("sha1")).cloned(),
                filename: f.filename.clone(),
                source: self.id().to_string(),
                version: v.version_number.clone(),
                dependencies: project_deps.get(&v.project_id).cloned().unwrap_or_default(),
            })
        }).collect();
        
        if let Some(root_id) = root_project_id {
            if let Some(pos) = targets.iter().position(|t| t.id == root_id) {
                let root_target = targets.remove(pos);
                targets.insert(0, root_target);
            }
        }

        Ok(targets)
    }
}
