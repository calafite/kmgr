use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use strsim::jaro_winkler;
use tokio::fs;

#[derive(Debug)]
pub struct KmgrState {
    pub default_mc_version: String,
    pub mod_loader: String,
    pub mods_folder: String,
    pub installed_mods: BTreeMap<String, InstalledMod>,
    pub profiles: BTreeMap<String, Vec<String>>,
    pub active_profile: String,
}

#[derive(Serialize, Deserialize)]
struct ConfigDto {
    #[serde(default = "default_mc_version_str")]
    default_mc_version: String,
    #[serde(default = "default_mod_loader_str")]
    mod_loader: String,
    #[serde(default = "default_mods_folder_str")]
    mods_folder: String,
    #[serde(default = "default_profiles")]
    profiles: BTreeMap<String, Vec<String>>,
    #[serde(default = "default_active_profile")]
    active_profile: String,
}

impl Default for ConfigDto {
    /// Returns the default configuration DTO.
    fn default() -> Self {
        Self {
            default_mc_version: default_mc_version_str(),
            mod_loader: default_mod_loader_str(),
            mods_folder: default_mods_folder_str(),
            profiles: default_profiles(),
            active_profile: default_active_profile(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct LockDto {
    #[serde(default)]
    installed_mods: BTreeMap<String, InstalledMod>,
}

impl Default for KmgrState {
    /// Returns the default application state.
    fn default() -> Self {
        Self {
            default_mc_version: default_mc_version_str(),
            mod_loader: default_mod_loader_str(),
            mods_folder: default_mods_folder_str(),
            installed_mods: BTreeMap::new(),
            profiles: default_profiles(),
            active_profile: default_active_profile(),
        }
    }
}

/// Returns an empty string as the default Minecraft version.
fn default_mc_version_str() -> String {
    "".to_string()
}

/// Returns an empty string as the default mod loader.
fn default_mod_loader_str() -> String {
    "".to_string()
}

/// Returns "mods" as the default mods folder path.
fn default_mods_folder_str() -> String {
    "mods".to_string()
}

/// Returns true as a default boolean value.
fn default_true() -> bool {
    true
}

/// Returns a default profiles map containing only the "default" profile.
fn default_profiles() -> BTreeMap<String, Vec<String>> {
    let mut m = BTreeMap::new();
    m.insert("default".to_string(), Vec::new());
    m
}

/// Returns "default" as the default active profile name.
fn default_active_profile() -> String {
    "default".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstalledMod {
    pub name: String,
    pub version: String,
    pub source: String,
    pub filename: String,
    #[serde(default)]
    pub download_url: String,
    pub hash: Option<String>,
    #[serde(default = "default_true")]
    pub is_explicit: bool,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl KmgrState {
    /// Loads application state configuration.
    ///
    /// Reads the state from the configuration file and lockfile, provisioning
    /// backfills for outdated profile schemas or uninitialized default layouts.
    pub async fn load() -> Result<Self> {
        let config: ConfigDto = match fs::read_to_string("kmgr.toml").await {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|_| ConfigDto::default()),
            Err(_) => ConfigDto::default(),
        };

        let lock: LockDto = match fs::read_to_string("kmgr.lock").await {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|_| LockDto::default()),
            Err(_) => LockDto::default(),
        };

        let mut state = Self {
            default_mc_version: config.default_mc_version,
            mod_loader: config.mod_loader,
            mods_folder: config.mods_folder,
            installed_mods: lock.installed_mods,
            profiles: config.profiles,
            active_profile: config.active_profile,
        };

        if !state.profiles.contains_key(&state.active_profile) {
            state
                .profiles
                .insert(state.active_profile.clone(), Vec::new());
        }

        if state.profiles.len() == 1 && state.active_profile == "default" {
            if let Some(default_profile) = state.profiles.get_mut("default") {
                if default_profile.is_empty() && !state.installed_mods.is_empty() {
                    for (id, m) in &state.installed_mods {
                        if m.is_explicit {
                            default_profile.push(id.clone());
                        }
                    }
                }
            }
        }

        Ok(state)
    }

    /// Checks if the environment is initialized with standard configurations.
    pub fn check_initialized(&self) -> Result<()> {
        if self.default_mc_version.is_empty() {
            anyhow::bail!(
                "Minecraft version is not configured. Run `kmgr setup` or `kmgr init` first."
            );
        }
        if self.mod_loader.is_empty() {
            anyhow::bail!("Mod loader is not configured. Run `kmgr setup` or `kmgr init` first.");
        }
        Ok(())
    }

    /// Resolves target package references.
    ///
    /// Takes a package reference name and matches it against the index of active
    /// installations, returning the respective identifier.
    pub fn find_mod_id(&self, mod_name: &str) -> Option<String> {
        for (id, mod_info) in &self.installed_mods {
            if id == mod_name || mod_info.name == mod_name {
                return Some(id.clone());
            }
        }
        None
    }

    /// Fuzzy-matches a query against installed mod names and ids.
    ///
    /// Returns the best `(id, display_name)` pair above a similarity threshold,
    /// preferring exact > case-insensitive > substring > edit-distance matches.
    pub fn find_mod_id_fuzzy(&self, query: &str) -> Option<(String, String)> {
        let q = query.to_lowercase();

        for (id, m) in &self.installed_mods {
            if id == query || m.name == query {
                return Some((id.clone(), m.name.clone()));
            }
        }

        for (id, m) in &self.installed_mods {
            if id.to_lowercase() == q || m.name.to_lowercase() == q {
                return Some((id.clone(), m.name.clone()));
            }
        }

        let mut best: Option<(String, String, f64)> = None;

        for (id, m) in &self.installed_mods {
            let name_lc = m.name.to_lowercase();
            let id_lc = id.to_lowercase();

            let score = if name_lc.contains(&q) || id_lc.contains(&q) {
                0.85
            } else {
                let name_score = jaro_winkler(&q, &name_lc);
                let id_score = jaro_winkler(&q, &id_lc);
                name_score.max(id_score)
            };

            if score >= 0.5 {
                if best.as_ref().map_or(true, |(_, _, s)| score > *s) {
                    best = Some((id.clone(), m.name.clone(), score));
                }
            }
        }

        best.map(|(id, name, _)| (id, name))
    }
    /// Computes full deployment locations for packages.
    ///
    /// Takes a target filename and an activation flag. Interpolates the final
    /// location across the deployment scope.
    pub fn get_mod_path(&self, filename: &str, enabled: bool) -> String {
        let mut dest = std::path::Path::new(&self.mods_folder).join(filename);
        if !enabled {
            let mut ext = dest.into_os_string();
            ext.push(".disabled");
            dest = std::path::PathBuf::from(ext);
        }
        dest.to_string_lossy().to_string()
    }

    /// Extends configuration to durable storage.
    ///
    /// Serializes the profile constraints and physical deployments into separate
    /// lock and context files using temporary atomic writes.
    pub async fn save(&self) -> Result<()> {
        let config = ConfigDto {
            default_mc_version: self.default_mc_version.clone(),
            mod_loader: self.mod_loader.clone(),
            mods_folder: self.mods_folder.clone(),
            profiles: self.profiles.clone(),
            active_profile: self.active_profile.clone(),
        };

        let lock = LockDto {
            installed_mods: self.installed_mods.clone(),
        };

        let config_out = toml::to_string_pretty(&config)?;
        let lock_out = toml::to_string_pretty(&lock)?;

        atomic_write("kmgr.toml", &config_out).await?;
        atomic_write("kmgr.lock", &lock_out).await?;

        Ok(())
    }
}

/// Writes content to a file atomically by writing to a temporary file first and then renaming it.
async fn atomic_write(path: &str, content: &str) -> Result<()> {
    let tmp_path = format!("{}.tmp", path);
    fs::write(&tmp_path, content).await?;
    fs::rename(&tmp_path, path).await?;
    Ok(())
}
