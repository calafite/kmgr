use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct ProviderSearchResult {
    pub title: String,
    pub description: String,
    pub id_or_slug: String,
    pub extra: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub id: String,
    pub name: String,
    pub download_url: String,
    pub hash: Option<String>,
    pub filename: String,
    pub source: String,
    pub version: String,
    pub dependencies: Vec<String>,
}

#[async_trait]
pub trait ModProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    
    async fn search(&self, query: &str) -> Result<Vec<ProviderSearchResult>>;
    async fn resolve(&self, project: &str, mc_version: &str, loader: &str) -> Result<Vec<ResolvedTarget>>;
}
