use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::fs;

#[derive(Debug)]
pub struct KmgrState {
    pub default_mc_version: String,
    pub mod_loader: String,
    pub mods_folder: String,
    pub installed_mods: HashMap<String, InstalledMod>,
    pub profiles: HashMap<String, Vec<String>>,
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
    profiles: HashMap<String, Vec<String>>,
    #[serde(default = "default_active_profile")]
    active_profile: String,
}

impl Default for ConfigDto {
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
    installed_mods: HashMap<String, InstalledMod>,
}

impl Default for KmgrState {
    fn default() -> Self {
        Self {
            default_mc_version: default_mc_version_str(),
            mod_loader: default_mod_loader_str(),
            mods_folder: default_mods_folder_str(),
            installed_mods: HashMap::new(),
            profiles: default_profiles(),
            active_profile: default_active_profile(),
        }
    }
}

fn default_mc_version_str() -> String { "".to_string() }
fn default_mod_loader_str() -> String { "".to_string() }
fn default_mods_folder_str() -> String { "mods".to_string() }
fn default_true() -> bool { true }
fn default_empty_vec() -> Vec<String> { Vec::new() }
fn default_profiles() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    m.insert("default".to_string(), Vec::new());
    m
}
fn default_active_profile() -> String { "default".to_string() }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstalledMod {
    pub name: String,
    pub version: String,
    pub source: String,
    pub filename: String,
    #[serde(default = "default_empty_string")]
    pub download_url: String,
    pub hash: Option<String>,
    #[serde(default = "default_true")]
    pub is_explicit: bool,
    #[serde(default = "default_empty_vec")]
    pub dependencies: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_empty_string() -> String { "".to_string() }

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
            state.profiles.insert(state.active_profile.clone(), Vec::new());
        }

        if state.profiles.len() == 1 && state.active_profile == "default" {
            let default_profile = state.profiles.get_mut("default").unwrap();
            if default_profile.is_empty() && !state.installed_mods.is_empty() {
                for (id, m) in &state.installed_mods {
                    if m.is_explicit {
                        default_profile.push(id.clone());
                    }
                }
            }
        }
        
        Ok(state)
    }

    /// Checks if the environment is initialized with standard configurations.
    pub fn check_initialized(&self) -> Result<()> {
        if self.default_mc_version.is_empty() {
            anyhow::bail!("Minecraft version is not configured. Run `kmgr setup` or `kmgr init` first.");
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
    
    /// Computes full deployment locations for packages.
    ///
    /// Takes a target filename and an activation flag. Interpolates the final
    /// location across the deployment scope.
    pub fn get_mod_path(&self, filename: &str, enabled: bool) -> String {
        let mut dest = format!("{}/{}", self.mods_folder, filename);
        if !enabled {
            dest.push_str(".disabled");
        }
        dest
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

async fn atomic_write(path: &str, content: &str) -> Result<()> {
    let tmp_path = format!("{}.tmp", path);
    fs::write(&tmp_path, content).await?;
    fs::rename(&tmp_path, path).await?;
    Ok(())
}

