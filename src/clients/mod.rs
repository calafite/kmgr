pub mod modrinth;
pub mod sourceforge;

use crate::core::registry::ProviderRegistry;
use anyhow::Result;

/// Builds and returns a provider registry populated with default clients.
pub fn build_registry() -> Result<ProviderRegistry> {
    let mut reg = ProviderRegistry::new();
    reg.register(Box::new(modrinth::ModrinthClient::new()?));
    reg.register(Box::new(sourceforge::SourceForgeClient::new()?));
    Ok(reg)
}
