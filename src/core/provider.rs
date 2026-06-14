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
    /// Returns the unique identifier of the provider.
    fn id(&self) -> &'static str;

    /// Returns the user-friendly display name of the provider.
    fn display_name(&self) -> &'static str;

    /// Searches for packages matching the query.
    async fn search(&self, query: &str) -> Result<Vec<ProviderSearchResult>>;

    /// Resolves a package and its dependencies for a specific Minecraft version and loader.
    async fn resolve(
        &self,
        project: &str,
        mc_version: &str,
        loader: &str,
        concurrency: usize,
    ) -> Result<Vec<ResolvedTarget>>;
}
