//! Integration tests for marketplace CLI commands

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Test search command
#[test]
fn test_search_command() {
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.arg("search")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("Found"))
        .stdout(predicate::str::contains("json-transformer"));
}

/// Test search with no results
#[test]
fn test_search_no_results() {
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.arg("search")
        .arg("nonexistent-plugin-xyz")
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins found"));
}

/// Test info command
#[test]
fn test_info_command() {
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.arg("info")
        .arg("json-transformer")
        .assert()
        .success()
        .stdout(predicate::str::contains("json-transformer"))
        .stdout(predicate::str::contains("Version:"))
        .stdout(predicate::str::contains("Description:"))
        .stdout(predicate::str::contains("Author:"));
}

/// Test info command with non-existent plugin
#[test]
fn test_info_nonexistent_plugin() {
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.arg("info")
        .arg("nonexistent-plugin")
        .assert()
        .failure();
}

/// Test list command with no plugins
#[test]
fn test_list_empty() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("dataflare").join("plugins");
    fs::create_dir_all(&cache_dir).unwrap();

    // Set environment variable to use temp cache directory
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins installed"));
}

/// Test install and list workflow
#[test]
fn test_install_list_workflow() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();

    // Install a plugin
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("install")
        .arg("json-transformer")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed successfully"));

    // List plugins to verify installation
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("json-transformer"));
}

/// Test install, list, and remove workflow
#[test]
fn test_full_plugin_lifecycle() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();

    // Install a plugin
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("install")
        .arg("json-transformer")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed successfully"));

    // List plugins to verify installation
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("json-transformer"));

    // Remove the plugin
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("remove")
        .arg("json-transformer")
        .assert()
        .success()
        .stdout(predicate::str::contains("removed successfully"));

    // Verify plugin is removed
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins installed"));
}

/// Test detailed list command
#[test]
fn test_detailed_list() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();

    // Install a plugin
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("install")
        .arg("json-transformer")
        .assert()
        .success();

    // List plugins with detailed info
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("list")
        .arg("--detailed")
        .assert()
        .success()
        .stdout(predicate::str::contains("json-transformer"))
        .stdout(predicate::str::contains("Installed:"))
        .stdout(predicate::str::contains("Source:"))
        .stdout(predicate::str::contains("Path:"));
}

/// Test update command
#[test]
fn test_update_command() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();

    // Install a plugin first
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("install")
        .arg("json-transformer")
        .assert()
        .success();

    // Update the plugin
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("update")
        .arg("json-transformer")
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

/// Test update all plugins
#[test]
fn test_update_all() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();

    // Install a plugin first
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("install")
        .arg("json-transformer")
        .assert()
        .success();

    // Update all plugins
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.env("DATAFLARE_CACHE_DIR", temp_dir.path())
        .arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

/// Test help command
#[test]
fn test_help_command() {
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("DataFlare WASM Plugin CLI Tool"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("remove"));
}

/// Test version command
#[test]
fn test_version_command() {
    let mut cmd = Command::cargo_bin("dataflare-plugin").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}
