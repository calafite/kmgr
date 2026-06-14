pub mod modrinth;
use crate::core::registry::ProviderRegistry;
use anyhow::Result;

pub fn build_registry() -> Result<ProviderRegistry> {
    let mut reg = ProviderRegistry::new();
    reg.register(Box::new(modrinth::ModrinthClient::new()?));
    Ok(reg)
}
