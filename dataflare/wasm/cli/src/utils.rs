//! Utility functions for the CLI

pub mod spinner;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write};
use colored::*;

/// File system utilities
pub struct FileUtils;

impl FileUtils {
    /// Copy directory recursively
    pub fn copy_dir_recursive<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
        let src = src.as_ref();
        let dst = dst.as_ref();

        if !src.is_dir() {
            anyhow::bail!("Source is not a directory: {}", src.display());
        }

        fs::create_dir_all(dst)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                Self::copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }

    /// Get file size in human readable format
    pub fn format_file_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    /// Check if file is executable
    pub fn is_executable<P: AsRef<Path>>(path: P) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(path) {
                let permissions = metadata.permissions();
                permissions.mode() & 0o111 != 0
            } else {
                false
            }
        }

        #[cfg(windows)]
        {
            // On Windows, check file extension
            let path = path.as_ref();
            if let Some(ext) = path.extension() {
                matches!(ext.to_str(), Some("exe") | Some("bat") | Some("cmd"))
            } else {
                false
            }
        }
    }
}

/// Terminal utilities
pub struct TerminalUtils;

impl TerminalUtils {
    /// Ask user for confirmation
    pub fn confirm(message: &str) -> Result<bool> {
        print!("{} [y/N]: ", message.yellow());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        Ok(input == "y" || input == "yes")
    }

    /// Ask user for input
    pub fn prompt(message: &str) -> Result<String> {
        print!("{}: ", message.yellow());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input.trim().to_string())
    }

    /// Ask user for input with default value
    pub fn prompt_with_default(message: &str, default: &str) -> Result<String> {
        print!("{} [{}]: ", message.yellow(), default.cyan());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim();
        if input.is_empty() {
            Ok(default.to_string())
        } else {
            Ok(input.to_string())
        }
    }

    /// Print success message
    pub fn success(message: &str) {
        println!("{} {}", "✅".green(), message);
    }

    /// Print warning message
    pub fn warning(message: &str) {
        println!("{} {}", "⚠️".yellow(), message.yellow());
    }

    /// Print error message
    pub fn error(message: &str) {
        println!("{} {}", "❌".red(), message.red());
    }

    /// Print info message
    pub fn info(message: &str) {
        println!("{} {}", "ℹ️".blue(), message);
    }
}

/// Version utilities
pub struct VersionUtils;

impl VersionUtils {
    /// Parse and validate semantic version
    pub fn parse_version(version: &str) -> Result<semver::Version> {
        semver::Version::parse(version)
            .map_err(|e| anyhow::anyhow!("Invalid version format '{}': {}", version, e))
    }

    /// Check if version satisfies requirement
    pub fn satisfies_requirement(version: &str, requirement: &str) -> Result<bool> {
        let version = Self::parse_version(version)?;
        let req = semver::VersionReq::parse(requirement)
            .map_err(|e| anyhow::anyhow!("Invalid version requirement '{}': {}", requirement, e))?;

        Ok(req.matches(&version))
    }

    /// Get next version based on increment type
    pub fn increment_version(version: &str, increment: VersionIncrement) -> Result<String> {
        let mut version = Self::parse_version(version)?;

        match increment {
            VersionIncrement::Major => {
                version.major += 1;
                version.minor = 0;
                version.patch = 0;
            }
            VersionIncrement::Minor => {
                version.minor += 1;
                version.patch = 0;
            }
            VersionIncrement::Patch => {
                version.patch += 1;
            }
        }

        Ok(version.to_string())
    }
}

/// Version increment types
#[derive(Debug, Clone, Copy)]
pub enum VersionIncrement {
    Major,
    Minor,
    Patch,
}

/// Network utilities
pub struct NetworkUtils;

impl NetworkUtils {
    /// Check if URL is reachable
    pub async fn is_url_reachable(url: &str) -> bool {
        match reqwest::get(url).await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Download file from URL
    pub async fn download_file<P: AsRef<Path>>(url: &str, path: P) -> Result<()> {
        let response = reqwest::get(url).await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download file: HTTP {}", response.status());
        }

        let content = response.bytes().await?;
        fs::write(path, content)?;

        Ok(())
    }

    /// Get content from URL as string
    pub async fn get_content(url: &str) -> Result<String> {
        let response = reqwest::get(url).await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get content: HTTP {}", response.status());
        }

        Ok(response.text().await?)
    }
}

/// Process utilities
pub struct ProcessUtils;

impl ProcessUtils {
    /// Check if command is available
    pub fn command_exists(command: &str) -> bool {
        which::which(command).is_ok()
    }

    /// Run command and capture output
    pub fn run_command(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<std::process::Output> {
        let mut cmd = std::process::Command::new(command);
        cmd.args(args);

        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }

        cmd.output()
            .map_err(|e| anyhow::anyhow!("Failed to run command '{}': {}", command, e))
    }

    /// Run command and check if it succeeds
    pub fn run_command_success(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<bool> {
        let output = Self::run_command(command, args, cwd)?;
        Ok(output.status.success())
    }
}
