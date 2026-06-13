use super::provider::ModProvider;
use anyhow::{Result, anyhow};
use std::collections::HashMap;

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn ModProvider>>,
}

impl ProviderRegistry {
    /// Creates a new empty ProviderRegistry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Registers a new provider in the registry.
    pub fn register(&mut self, provider: Box<dyn ModProvider>) {
        self.providers
            .insert(provider.id().to_lowercase(), provider);
    }

    /// Retrieves a provider by its identifier.
    pub fn get(&self, id: &str) -> Result<&dyn ModProvider> {
        let key = id.to_lowercase();
        self.providers
            .get(&key)
            .map(|p| p.as_ref())
            .ok_or_else(|| anyhow!("Provider '{}' not found", id))
    }

    /// Retrieves the default provider (Modrinth).
    pub fn get_default(&self) -> Result<&dyn ModProvider> {
        self.get("modrinth")
    }
}
