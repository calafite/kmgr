#![allow(dead_code)]

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use crate::core::provider::{ModProvider, ProviderSearchResult, ResolvedTarget};

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
        format!("https://sourceforge.net/projects/{}/files/latest/download", project_name)
    }
}

impl ModProvider for SourceForgeClient {
    /// Returns the unique identifier for the SourceForge provider.
    fn id(&self) -> &'static str { "sourceforge" }

    /// Returns the display name of the SourceForge provider.
    fn display_name(&self) -> &'static str { "SourceForge" }
    
    /// Searches for projects on SourceForge matching the query.
    fn search<'a>(&'a self, query: &'a str) -> futures::future::BoxFuture<'a, Result<Vec<ProviderSearchResult>>> {
        Box::pin(async move {
            let normalized_query = query.to_lowercase().replace(' ', "-");
            match self.get_project(&normalized_query).await {
                Ok(p) => {
                    let mut results = vec![];
                    results.push(ProviderSearchResult {
                        title: p.name,
                        description: p.shortdesc.unwrap_or_default(),
                        id_or_slug: normalized_query,
                        extra: p.url,
                    });
                    Ok(results)
                }
                Err(_) => {
                    Ok(vec![])
                }
            }
        })
    }

    /// Resolves a project to its latest download target.
    fn resolve<'a>(&'a self, project_name: &'a str, _mc_version: &'a str, _loader: &'a str) -> futures::future::BoxFuture<'a, Result<Vec<ResolvedTarget>>> {
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
