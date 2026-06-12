use super::provider::ModProvider;
use anyhow::{Result, anyhow};
use std::collections::HashMap;

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn ModProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn ModProvider>) {
        self.providers
            .insert(provider.id().to_lowercase(), provider);
    }

    pub fn get(&self, id: &str) -> Result<&dyn ModProvider> {
        let mut key = id.to_lowercase();
        if key == "sf" {
            key = "sourceforge".to_string();
        }

        self.providers
            .get(&key)
            .map(|p| p.as_ref())
            .ok_or_else(|| anyhow!("Provider '{}' not found", id))
    }

    pub fn get_default(&self) -> Result<&dyn ModProvider> {
        self.get("modrinth")
    }
}
