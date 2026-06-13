#![allow(dead_code)]

use crate::core::provider::{ModProvider, ProviderSearchResult, ResolvedTarget};
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

pub struct SourceForgeClient {
    client: Client,
    base_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SfProject {
    pub name: String,
    pub shortdesc: Option<String>,
    pub url: Option<String>,
}

impl SourceForgeClient {
    /// Creates a new instance of the SourceForge API client.
    pub fn new() -> Self {
        SourceForgeClient {
            client: Client::builder().build().unwrap(),
            base_url: "https://sourceforge.net/rest/p".to_string(),
        }
    }

    /// Retrieves project details from SourceForge using its project name.
    pub async fn get_project(&self, project_name: &str) -> Result<SfProject> {
        let url = format!("{}/{}", self.base_url, project_name);

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("Project '{}' not found on SourceForge", project_name);
        }

        Ok(response.error_for_status()?.json().await?)
    }

    /// Generates the latest download URL for a given SourceForge project.
    pub fn get_latest_download_url(&self, project_name: &str) -> String {
        format!(
            "https://sourceforge.net/projects/{}/files/latest/download",
            project_name
        )
    }
}

impl ModProvider for SourceForgeClient {
    /// Returns the unique identifier for the SourceForge provider.
    fn id(&self) -> &'static str {
        "sourceforge"
    }

    /// Returns the display name of the SourceForge provider.
    fn display_name(&self) -> &'static str {
        "SourceForge"
    }

    /// Searches for projects on SourceForge matching the query.
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> futures::future::BoxFuture<'a, Result<Vec<ProviderSearchResult>>> {
        let query_owned = query.to_string();
        Box::pin(async move {
            let url = format!(
                "https://sourceforge.net/directory/?q={}",
                query_owned.replace(' ', "+")
            );

            let html = match self.client.get(&url).send().await {
                Ok(resp) => resp.text().await.unwrap_or_default(),
                Err(_) => String::new(),
            };

            let mut results: Vec<ProviderSearchResult> = vec![];
            let mut current = html.as_str();

            while let Some(idx) = current.find("href=\"/projects/") {
                current = &current[idx + 16..];

                if let Some(slug_end) = current.find("/") {
                    let slug = current[..slug_end].to_string();
                    if slug.is_empty()
                        || slug.contains('"')
                        || slug.contains('?')
                        || slug == "search"
                    {
                        continue;
                    }

                    if results.iter().any(|r| r.id_or_slug == slug) {
                        continue;
                    }

                    let mut title = slug.clone();
                    let mut description = String::new();

                    let block_end = current.find("</li>").unwrap_or(current.len().min(1500));
                    let block = &current[..block_end];

                    if let Some(title_start) = block.find("itemprop=\"name\">") {
                        let temp = &block[title_start + 16..];
                        if let Some(title_end) = temp.find('<') {
                            title = temp[..title_end].trim().to_string();
                        }
                    }

                    if let Some(desc_start) = block.find("itemprop=\"description\">") {
                        let temp = &block[desc_start + 23..];
                        if let Some(desc_end) = temp.find('<') {
                            description = temp[..desc_end].trim().to_string();
                        }
                    }

                    title = title
                        .replace("&quot;", "\"")
                        .replace("&#39;", "'")
                        .replace("&amp;", "&");
                    description = description
                        .replace("&quot;", "\"")
                        .replace("&#39;", "'")
                        .replace("&amp;", "&");

                    results.push(ProviderSearchResult {
                        title,
                        description,
                        id_or_slug: slug,
                        extra: None,
                    });

                    if results.len() >= 10 {
                        break;
                    }
                }
            }

            if results.is_empty() {
                let normalized_query = query_owned.to_lowercase().replace(' ', "-");
                if let Ok(p) = self.get_project(&normalized_query).await {
                    results.push(ProviderSearchResult {
                        title: p.name,
                        description: p.shortdesc.unwrap_or_default(),
                        id_or_slug: normalized_query,
                        extra: p.url,
                    });
                }
            }

            Ok(results)
        })
    }

    /// Resolves a project to its latest download target.
    fn resolve<'a>(
        &'a self,
        project_name: &'a str,
        _mc_version: &'a str,
        _loader: &'a str,
    ) -> futures::future::BoxFuture<'a, Result<Vec<ResolvedTarget>>> {
        Box::pin(async move {
            let normalized_name = project_name.to_lowercase().replace(' ', "-");
            let _project = self.get_project(&normalized_name).await?;

            let download_url = self.get_latest_download_url(&normalized_name);

            Ok(vec![ResolvedTarget {
                id: normalized_name.clone(),
                name: _project.name,
                download_url,
                hash: None,
                filename: format!("{}-latest.jar", normalized_name),
                source: self.id().to_string(),
                version: "latest".to_string(),
                dependencies: vec![],
            }])
        })
    }
}
