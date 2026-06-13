use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kmgr")]
#[command(author, version, about = " A fast, lightweight Minecraft CLI Mod Manager written in Rust", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new modpack or mod manager configuration
    Init {
        /// Minecraft version (e.g., 1.20.1)
        #[arg(short, long)]
        mc_version: String,

        /// Mod loader (e.g., fabric, forge, neoforge, quilt)
        #[arg(short, long)]
        loader: String,
    },

    /// Interactively setup the configuration
    Setup,

    /// Search for a mod across platforms
    Search {
        /// The query to search for
        query: String,

        /// Source to search in (modrinth, sf) - Defaults to modrinth
        #[arg(short, long)]
        source: Option<String>,
    },

    /// Install specific mods by name
    Install {
        /// Names or Slugs of the mods
        #[arg(required = true)]
        mods: Vec<String>,

        /// Minecraft version (e.g., 1.20.1) - Optional
        #[arg(short, long)]
        mc_version: Option<String>,

        /// Source to install from (modrinth, sf) - Defaults to modrinth
        #[arg(short, long)]
        source: Option<String>,
    },

    /// Update all installed mods
    Update {
        /// Apply the updates (download the new versions)
        #[arg(short, long)]
        apply: bool,
    },

    /// Remove installed mods
    Remove {
        /// Names of the mods to remove
        #[arg(required = true)]
        mods: Vec<String>,
    },

    /// Enable disabled mods
    Enable {
        /// Names of the mods to enable
        #[arg(required = true)]
        mods: Vec<String>,
    },

    /// Disable active mods
    Disable {
        /// Names of the mods to disable
        #[arg(required = true)]
        mods: Vec<String>,
    },

    /// Fetch and download missing mod files according to kmgr.toml
    Sync,

    /// Remove unused dependencies
    Prune,

    /// Manage mod profiles
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },

    /// List currently installed mods
    List,
}

#[derive(Subcommand)]
pub enum ProfileCommands {
    /// List all profiles
    List,
    /// Create a new profile
    Create { name: String },
    /// Switch to a profile
    Switch { name: String },
    /// Add existing mods to the current profile
    Add {
        #[arg(required = true)]
        mods: Vec<String>,
    },
    /// Remove mods from the current profile
    Remove {
        #[arg(required = true)]
        mods: Vec<String>,
    },
    /// Delete a profile
    Delete { name: String },
    /// Rename a profile
    Rename { old_name: String, new_name: String },
}
