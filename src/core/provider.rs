use anyhow::Result;

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

pub trait ModProvider: Send + Sync {
    /// Returns the unique identifier of the provider.
    fn id(&self) -> &'static str;

    /// Returns the user-friendly display name of the provider.
    fn display_name(&self) -> &'static str;
    
    /// Searches for packages matching the query.
    fn search<'a>(&'a self, query: &'a str) -> futures::future::BoxFuture<'a, Result<Vec<ProviderSearchResult>>>;

    /// Resolves a package and its dependencies for a specific Minecraft version and loader.
    fn resolve<'a>(&'a self, project: &'a str, mc_version: &'a str, loader: &'a str) -> futures::future::BoxFuture<'a, Result<Vec<ResolvedTarget>>>;
}
