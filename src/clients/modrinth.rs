use crate::core::provider::{ModProvider, ProviderSearchResult, ResolvedTarget};
use anyhow::Result;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::{Client, header};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
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
    pub fn new() -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("kmgr/0.1.0"),
        );

        Ok(ModrinthClient {
            client: Client::builder()
                .default_headers(headers)
                .timeout(std::time::Duration::from_secs(crate::core::utils::HTTP_TIMEOUT_SECS))
                .build()?,
            base_url: "https://api.modrinth.com/v2".to_string(),
        })
    }

    /// Handles GET requests with automatic retries for HTTP 429 Too Many Requests
    async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut attempts = 0;
        let max_attempts = 5;

        loop {
            let response = self.client.get(url).send().await?;

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if attempts >= max_attempts {
                    anyhow::bail!("Modrinth API rate limit exceeded. Please try again later.");
                }

                let delay_secs = response
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or_else(|| 2_u64.pow(attempts));

                tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs + 1)).await;
                attempts += 1;
                continue;
            }

            return Ok(response);
        }
    }

    pub async fn search_mods_internal(&self, query: &str) -> Result<SearchResponse> {
        let mut url = reqwest::Url::parse(&format!("{}/search", self.base_url))?;
        url.query_pairs_mut().append_pair("query", query);

        let response = self
            .get_with_retry(url.as_str())
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    pub async fn get_project(&self, id_or_slug: &str) -> Result<Project> {
        let url = format!("{}/project/{}", self.base_url, id_or_slug);
        let response = self.get_with_retry(&url).await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("Project '{}' not found on Modrinth", id_or_slug);
        }

        Ok(response.error_for_status()?.json().await?)
    }

    pub async fn get_versions(
        &self,
        project_id: &str,
        mc_version: &str,
        loader: &str,
    ) -> Result<Vec<Version>> {
        let game_versions = format!("[\"{}\"]", mc_version);
        let loaders = format!("[\"{}\"]", loader);

        let mut url =
            reqwest::Url::parse(&format!("{}/project/{}/version", self.base_url, project_id))?;
        url.query_pairs_mut()
            .append_pair("game_versions", &game_versions)
            .append_pair("loaders", &loaders);

        let response = self
            .get_with_retry(url.as_str())
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    pub async fn get_version(&self, version_id: &str) -> Result<Version> {
        let url = format!("{}/version/{}", self.base_url, version_id);
        let response = self.get_with_retry(&url).await?.error_for_status()?;
        Ok(response.json().await?)
    }
}

#[async_trait]
impl ModProvider for ModrinthClient {
    fn id(&self) -> &'static str {
        "modrinth"
    }

    fn display_name(&self) -> &'static str {
        "Modrinth"
    }

    async fn search(&self, query: &str) -> Result<Vec<ProviderSearchResult>> {
        let response = self.search_mods_internal(query).await?;

        let mapped = response
            .hits
            .into_iter()
            .map(|h| ProviderSearchResult {
                title: h.title,
                description: h.description,
                id_or_slug: h.slug,
                extra: Some(format!("Project Type: {}", h.project_type)),
            })
            .collect();

        Ok(mapped)
    }

    async fn resolve(
        &self,
        project_slug: &str,
        mc_version: &str,
        loader: &str,
        concurrency: usize,
    ) -> Result<Vec<ResolvedTarget>> {
        let (actual_slug, requested_version) =
            if let Some((name, ver)) = project_slug.split_once('@') {
                (name, Some(ver.to_string()))
            } else {
                (project_slug, None)
            };

        let mut resolved_versions: HashMap<String, Version> = HashMap::new();
        let mut seen_projects: HashSet<String> = HashSet::new();
        let mut project_names: HashMap<String, String> = HashMap::new();
        let mut project_deps: HashMap<String, Vec<String>> = HashMap::new();

        let mut root_project_id = None;

        let mut queue: Vec<(String, Option<String>)> =
            vec![(actual_slug.to_string(), requested_version)];

        let concurrency_limit = concurrency;

        while !queue.is_empty() {
            let current_batch: Vec<(String, Option<String>)> = queue
                .drain(..)
                .filter(|(req, _)| !seen_projects.contains(req))
                .collect();

            if current_batch.is_empty() {
                break;
            }

            for (req, _) in &current_batch {
                seen_projects.insert(req.clone());
            }

            let client_clone = self.clone();
            let mc_version_str = mc_version.to_string();
            let loader_str = loader.to_string();

            let mut stream = stream::iter(current_batch)
                .map(|(current_req, specific_version)| {
                    let client = client_clone.clone();
                    let mc_ver = mc_version_str.clone();
                    let lod = loader_str.clone();
                    async move {
                        let project = match client.get_project(&current_req).await {
                            Ok(p) => p,
                            Err(_) => return None,
                        };

                        let versions = match client.get_versions(&project.id, &mc_ver, &lod).await {
                            Ok(v) => v,
                            Err(_) => return None,
                        };

                        let target_version = if let Some(ver) = specific_version {
                            versions
                                .into_iter()
                                .find(|v| v.version_number == ver || v.id == ver)
                        } else {
                            versions.into_iter().next()
                        };

                        if let Some(target_version) = target_version {
                            let mut next_deps = Vec::new();
                            let mut deps_list = Vec::new();
                            for dep in &target_version.dependencies {
                                if dep.dependency_type == "required" {
                                    if let Some(dep_version_id) = &dep.version_id {
                                        if let Ok(v) = client.get_version(dep_version_id).await {
                                            next_deps
                                                .push((v.project_id.clone(), Some(v.id.clone())));
                                            deps_list.push(v.project_id.clone());
                                        }
                                    } else if let Some(dep_proj_id) = &dep.project_id {
                                        next_deps.push((dep_proj_id.clone(), None));
                                        deps_list.push(dep_proj_id.clone());
                                    }
                                }
                            }
                            Some((project, target_version, next_deps, deps_list))
                        } else {
                            None
                        }
                    }
                })
                .buffer_unordered(concurrency_limit);

            while let Some(res) = stream.next().await {
                if let Some((project, target_version, next_deps, deps_list)) = res {
                    if root_project_id.is_none() {
                        root_project_id = Some(project.id.clone());
                    }

                    seen_projects.insert(project.id.clone());
                    seen_projects.insert(project.slug.clone());

                    project_names.insert(project.id.clone(), project.title.clone());
                    project_deps.insert(project.id.clone(), deps_list);
                    resolved_versions.insert(project.id.clone(), target_version);

                    for dep_req in next_deps {
                        queue.push(dep_req);
                    }
                }
            }
        }

        let mut targets: Vec<ResolvedTarget> = resolved_versions
            .into_values()
            .filter_map(|v| {
                let file = v
                    .files
                    .iter()
                    .find(|f| f.primary)
                    .or_else(|| v.files.first());
                file.map(|f| ResolvedTarget {
                    id: v.project_id.clone(),
                    name: project_names
                        .get(&v.project_id)
                        .cloned()
                        .unwrap_or_else(|| v.project_id.clone()),
                    download_url: f.url.clone(),
                    hash: f
                        .hashes
                        .get("sha512")
                        .or_else(|| f.hashes.get("sha1"))
                        .cloned(),
                    filename: std::path::Path::new(&f.filename)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("unknown_mod.jar")
                        .to_string(),
                    source: self.id().to_string(),
                    version: v.version_number.clone(),
                    dependencies: project_deps.get(&v.project_id).cloned().unwrap_or_default(),
                })
            })
            .collect();

        if let Some(root_id) = root_project_id {
            if let Some(pos) = targets.iter().position(|t| t.id == root_id) {
                let root_target = targets.remove(pos);
                targets.insert(0, root_target);
            }
        }

        Ok(targets)
    }
}
