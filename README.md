# kmgr

`kmgr` is a high-performance, lightweight, and robust Command-Line Minecraft Mod Manager written in Rust. Designed to streamline modpack development, search, installation, and deployment, `kmgr` eliminates the overhead of typical mod management, offering surgical control over dependencies, active configurations, and environment profiles.

---

## Key Features

- **Concurrent Resolver**: Resolves and queries platform metadata concurrently (using `tokio` and `futures`), offering lightning-fast update checks.
- **Profiles Management**: Creates, switches, renames, and manages arbitrary mod sets (profiles) with zero manual file duplication. Switching profiles will seamlessly enable or disable physical disk artifacts.
- **Integrity & Checksums**: Downloads are validated against cryptographic checksum signatures (`SHA-1` or `SHA-512`). Syncing scans current file structures and re-downloads mismatching or corrupted assets on the fly.
- **Dependency Isolation**: Distinguishes between explicitly-requested mods and dependency-chained mods.
- **Pruner Engine**: Scans mod dependencies recursively, cleaning out unused downstream components that no longer trace back to any explicit root packages.

---

## Core Configuration Architecture

`kmgr` relies on two fundamental files created automatically in your project root to keep the local filesystem synchronized with desired configurations:

```
├── kmgr.toml       # Human-readable state configuration (Environment settings and Profile lists)
└── kmgr.lock       # Cryptographically validated package tracking database
```

### 1. `kmgr.toml`
This file defines global settings, profiles, and active configurations. It can be modified manually or built via the interactive `setup` wizard.

```toml
default_mc_version = "1.20.4"
mod_loader = "fabric"
mods_folder = "mods"
active_profile = "solo-exploration"

[profiles]
default = []
solo-exploration = ["sodium", "iris", "lithium"]
server-coop = ["sodium", "voicechat"]
```

### 2. `kmgr.lock`
This lockfile records every installed mod's exact version, origin, physical filename, cryptographic hash, dependencies, explicit-installation state, and enablement status. It prevents double-installations and acts as a single source of truth for the local environment state.

```toml
[installed_mods.sodium]
name = "Sodium"
version = "0.5.8"
source = "modrinth"
filename = "sodium-fabric-0.5.8+mc1.20.4.jar"
download_url = "https://cdn.modrinth.com/..."
hash = "cb339c..."
is_explicit = true
dependencies = []
enabled = true
```

---

## Getting Started

### Installation & Compilation

Make sure you have Rust and Cargo installed, then clone the repository and build:

```bash
cargo build --release
```

The resulting executable is generated at `target/release/kmgr`. You can move it or symlink it into your system's `PATH`.

---

## Interactive Wizard & Initialization

`kmgr` offers two routes to establish configuration files:

### 1. Interactive Setup
Run `setup` to build or update your configuration details interatively. It normalizes inputs and performs strict checks on path patterns and loaders.

```bash
kmgr setup
```

**Validation & Safe Normalization Logic**:
- **Minecraft Version**: Rejects empty strings, validates alphanumeric syntax, ensures numbers are present, and normalizes version string to lowercase.
- **Mod Loader**: Restriced to validated, compatible loaders (`fabric`, `forge`, `neoforge`, `quilt`).
- **Mods Folder**: Validates paths to prevent dangerous characters (e.g. `*`, `?`, `"`, `<`), normalizes directory-slashes (converting Windows backslashes `\` to Unix forward slashes `/`), strips trailing slashes, and safely initializes directory trees.

### 2. Manual CLI Initialization
For immediate zero-interaction setups, initialize the workspace configuration specifying standard version and loader details directly:

```bash
kmgr init --mc-version 1.20.4 --loader fabric
```

---

## Command Reference

Every command (except configuration and search commands) validates that the environment has been successfully configured first.

### Search
Query Modrinth or SourceForge databases for a specific mod query.

```bash
kmgr search "iris shadders" [-s <source>]
```
* **Options**:
  * `-s, --source`: The source target. Supported: `modrinth`, `sf` (SourceForge). Defaults to `modrinth`.

### Install
Download and install specific mods by name or slug. `kmgr` will automatically download downstream dependencies recursively.

```bash
kmgr install sodium iris [-m <mc_version>] [-s <source>]
```
* **Arguments**:
  * `mods`: Space-separated list of mod names or platform slugs.
* **Options**:
  * `-m, --mc-version`: Temporarily override the configured Minecraft version for this download.
  * `-s, --source`: Platform to fetch from. Defaults to `modrinth`.

### Update
Perform a concurrent check of all registered packages against update repositories.

```bash
# Check for updates without applying changes
kmgr update

# Query, download new versions, and deactivate older files
kmgr update --apply
```
* **Options**:
  * `-a, --apply`: Actually triggers the download of the newer versions, switching physical `.jar` allocations.

### Remove
Uninstall mods by name/slug.

```bash
kmgr remove iris
```

### Sync
Validates that physical files present on the filesystem match the exact metadata declared in `kmgr.lock`.

```bash
kmgr sync
```
* Safely downloads missing physical files using recorded CDN URLs.
* Computes local checksums of existing `.jar` files and compares them against lockfile-recorded hash values (`SHA-1` or `SHA-512`). Re-downloads files on checksum mismatches or corrupted sizes.

### Prune
Optimizes clean installations. Sweeps through installed dependencies and deletes orphaned mods that are no longer referenced by any explicit parent mod.

```bash
kmgr prune
```

### List
Displays current active profile info, configuration formats, and a tabulated index of installed packages, their activation statuses, versions, and origins.

```bash
kmgr list
```

### Enable / Disable
Enables or disables individual folders/mod files quickly without destroying metadata or delete-and-redownload cycles. 

```bash
kmgr disable sodium
kmgr enable sodium
```
* **Disabling**: Renames `mods/sodium.jar` to `mods/sodium.jar.disabled`.
* **Enabling**: Reverts `mods/sodium.jar.disabled` back to `mods/sodium.jar`.

---

## Profile-Switching Workflows

Multi-profile support allows swapping between specific mod setups (e.g. competitive vanilla client vs. singleplayer creative design) seamlessly:

```bash
# List all profiles
kmgr profile list

# Create a new profile
kmgr profile create server-coop

# Swapping environments
kmgr profile switch server-coop

# Associate installed mods to the active profile
kmgr profile add sodium voicechat

# Remove registered mods from the active profile
kmgr profile remove voicechat

# Rename or delete profiles
kmgr profile rename server-coop local-testing
kmgr profile delete local-testing
```

### Switch Mechanics
When switching profiles (e.g. from `solo` to `coop`):
1. `kmgr` resolves the union of dependencies of all mods associated with the target profile.
2. It deactivates any mod file that is active but not in the target set by renaming it with `.disabled`.
3. It reactivates any disabled mod file required by the target set by stripping the `.disabled` suffix.
