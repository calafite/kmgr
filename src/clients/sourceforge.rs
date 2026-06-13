use crate::core::provider::{ModProvider, ProviderSearchResult, ResolvedTarget};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
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
    pub fn new() -> Result<Self> {
        Ok(SourceForgeClient {
            client: Client::builder().build()?,
            base_url: "https://sourceforge.net/rest/p".to_string(),
        })
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

#[async_trait]
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
    async fn search(&self, query: &str) -> Result<Vec<ProviderSearchResult>> {
        let url = format!(
            "https://sourceforge.net/directory/?q={}",
            query.replace(' ', "+")
        );

        let mut results: Vec<ProviderSearchResult> = vec![];

        {
            let html_text = match self.client.get(&url).send().await {
                Ok(resp) => resp.text().await.unwrap_or_default(),
                Err(_) => return Ok(vec![]),
            };

            let document = Html::parse_document(&html_text);

            let result_item_selector = Selector::parse("li").unwrap();
            let link_selector = Selector::parse("a[href^='/projects/']").unwrap();
            let title_selector = Selector::parse("[itemprop='name']").unwrap();
            let desc_selector = Selector::parse("[itemprop='description']").unwrap();

            for element in document.select(&result_item_selector) {
                if let Some(link_element) = element.select(&link_selector).next() {
                    if let Some(href) = link_element.value().attr("href") {
                        // Extract slug from "/projects/my-mod-name/"
                        let parts: Vec<&str> = href.split('/').collect();
                        if parts.len() < 3 {
                            continue;
                        }

                        let slug = parts[2].to_string();
                        if slug.is_empty() || slug == "search" {
                            continue;
                        }

                        if results.iter().any(|r| r.id_or_slug == slug) {
                            continue;
                        }

                        let title = element
                            .select(&title_selector)
                            .next()
                            .map(|el| el.text().collect::<String>().trim().to_string())
                            .unwrap_or_else(|| slug.clone());

                        let description = element
                            .select(&desc_selector)
                            .next()
                            .map(|el| el.text().collect::<String>().trim().to_string())
                            .unwrap_or_default();

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
            }
        }

        if results.is_empty() {
            let normalized_query = query.to_lowercase().replace(' ', "-");
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
    }

    /// Resolves a project to its latest download target.
    async fn resolve(
        &self,
        project_name: &str,
        _mc_version: &str,
        _loader: &str,
    ) -> Result<Vec<ResolvedTarget>> {
        let actual_name = project_name
            .split_once('@')
            .map(|(n, _)| n)
            .unwrap_or(project_name);

        let normalized_name = actual_name.to_lowercase().replace(' ', "-");
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
    }
}
