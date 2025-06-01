//! CLI command implementations

pub mod new;
pub mod build;
pub mod test;
pub mod validate;
pub mod marketplace;
pub mod package;
pub mod benchmark;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;

/// Common utilities for commands
pub struct CommandUtils;

impl CommandUtils {
    /// Find the plugin configuration file
    pub fn find_plugin_config() -> Result<PathBuf> {
        let current_dir = std::env::current_dir()?;

        // Look for plugin.toml in current directory and parent directories
        let mut dir = current_dir.as_path();
        loop {
            let config_path = dir.join("plugin.toml");
            if config_path.exists() {
                return Ok(config_path);
            }

            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }

        anyhow::bail!("Could not find plugin.toml in current directory or any parent directory");
    }

    /// Get the plugin root directory
    pub fn get_plugin_root() -> Result<PathBuf> {
        let config_path = Self::find_plugin_config()?;
        Ok(config_path.parent().unwrap().to_path_buf())
    }

    /// Check if we're in a plugin project
    pub fn is_plugin_project() -> bool {
        Self::find_plugin_config().is_ok()
    }

    /// Create directory if it doesn't exist
    pub fn ensure_dir<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }

    /// Get the target directory for builds
    pub fn get_target_dir() -> Result<PathBuf> {
        let root = Self::get_plugin_root()?;
        Ok(root.join("target"))
    }

    /// Get the output directory for packages
    pub fn get_output_dir() -> Result<PathBuf> {
        let root = Self::get_plugin_root()?;
        Ok(root.join("dist"))
    }
}
