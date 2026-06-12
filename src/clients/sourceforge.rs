#![allow(dead_code)]

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use async_trait::async_trait;
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
    pub fn new() -> Self {
        SourceForgeClient {
            client: Client::builder().build().unwrap(),
            base_url: "https://sourceforge.net/rest/p".to_string(),
        }
    }

    pub async fn get_project(&self, project_name: &str) -> Result<SfProject> {
        let url = format!("{}/{}", self.base_url, project_name);
        
        let response = self.client.get(&url).send().await?;
        
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("Project '{}' not found on SourceForge", project_name);
        }
        
        Ok(response.error_for_status()?.json().await?)
    }
    
    pub fn get_latest_download_url(&self, project_name: &str) -> String {
        format!("https://sourceforge.net/projects/{}/files/latest/download", project_name)
    }
}

#[async_trait]
impl ModProvider for SourceForgeClient {
    fn id(&self) -> &'static str { "sourceforge" }
    fn display_name(&self) -> &'static str { "SourceForge" }
    
    async fn search(&self, query: &str) -> Result<Vec<ProviderSearchResult>> {
        match self.get_project(query).await {
            Ok(p) => {
                let mut results = vec![];
                results.push(ProviderSearchResult {
                    title: p.name,
                    description: p.shortdesc.unwrap_or_default(),
                    id_or_slug: query.to_string(),
                    extra: p.url,
                });
                Ok(results)
            }
            Err(_) => {
                Ok(vec![]) // Not found
            }
        }
    }

    async fn resolve(&self, project_name: &str, _mc_version: &str, _loader: &str) -> Result<Vec<ResolvedTarget>> {
        let _project = self.get_project(project_name).await?;
        
        let download_url = self.get_latest_download_url(project_name);
        
        Ok(vec![ResolvedTarget {
            id: project_name.to_string(),
            name: _project.name,
            download_url,
            hash: None,
            filename: format!("{}-latest.jar", project_name),
            source: self.id().to_string(),
            version: "latest".to_string(),
            dependencies: vec![],
        }])
    }
}
