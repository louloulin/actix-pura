//! Plugin project template system

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use handlebars::Handlebars;
use log::info;

/// Plugin template generator
pub struct PluginTemplate {
    handlebars: Handlebars<'static>,
}

impl PluginTemplate {
    /// Create a new template generator
    pub fn new() -> Result<Self> {
        let mut handlebars = Handlebars::new();

        // Register helper functions
        handlebars.register_helper("eq", Box::new(eq_helper));

        // Register built-in templates
        register_rust_templates(&mut handlebars)?;
        register_javascript_templates(&mut handlebars)?;
        register_go_templates(&mut handlebars)?;

        Ok(Self { handlebars })
    }

    /// Generate a new plugin project
    pub async fn generate_project(
        &self,
        output_dir: &Path,
        name: &str,
        lang: &str,
        plugin_type: &str,
        template: &str,
        features: &[String],
    ) -> Result<()> {
        info!("Generating {} project with {} template", lang, template);

        // Prepare template context
        let mut context = HashMap::new();
        context.insert("plugin_name", name);
        context.insert("plugin_type", plugin_type);
        context.insert("template", template);
        context.insert("language", lang);
        let description = format!("A {} plugin for DataFlare", plugin_type);
        let author = "Your Name <your.email@example.com>";
        context.insert("description", &description);
        context.insert("author", author);

        // Create Rust-compatible crate name (replace hyphens with underscores)
        let crate_name = name.replace("-", "_");
        context.insert("crate_name", &crate_name);

        // Add features
        let features_str = features.join(",");
        context.insert("features", &features_str);

        let has_ai = features.contains(&"ai".to_string()).to_string();
        let has_streaming = features.contains(&"streaming".to_string()).to_string();
        let has_batch = features.contains(&"batch".to_string()).to_string();

        context.insert("has_ai", &has_ai);
        context.insert("has_streaming", &has_streaming);
        context.insert("has_batch", &has_batch);

        // Generate based on language
        match lang {
            "rust" => self.generate_rust_project(output_dir, &context).await?,
            "javascript" => self.generate_javascript_project_from_files(output_dir, &context).await?,
            "go" => self.generate_go_project_from_files(output_dir, &context).await?,
            _ => anyhow::bail!("Unsupported language: {}", lang),
        }

        Ok(())
    }

    /// Generate Rust project
    async fn generate_rust_project(
        &self,
        output_dir: &Path,
        context: &HashMap<&str, &str>,
    ) -> Result<()> {
        // Create directory structure
        let src_dir = output_dir.join("src");
        let tests_dir = output_dir.join("tests");
        let examples_dir = output_dir.join("examples");
        let docs_dir = output_dir.join("docs");

        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&tests_dir)?;
        fs::create_dir_all(&examples_dir)?;
        fs::create_dir_all(&docs_dir)?;

        // Generate Cargo.toml
        let cargo_toml = self.handlebars.render("rust/Cargo.toml", context)?;
        fs::write(output_dir.join("Cargo.toml"), cargo_toml)?;

        // Generate lib.rs
        let lib_rs = self.handlebars.render("rust/lib.rs", context)?;
        fs::write(src_dir.join("lib.rs"), lib_rs)?;

        // Generate plugin implementation based on type
        let plugin_type = context.get("plugin_type").unwrap();
        let impl_template = format!("rust/{}.rs", plugin_type);
        if self.handlebars.get_template(&impl_template).is_some() {
            let impl_rs = self.handlebars.render(&impl_template, context)?;
            fs::write(src_dir.join(format!("{}.rs", plugin_type)), impl_rs)?;
        }

        // Generate tests
        let test_rs = self.handlebars.render("rust/tests.rs", context)?;
        fs::write(tests_dir.join("integration_tests.rs"), test_rs)?;

        // Generate example
        let example_rs = self.handlebars.render("rust/example.rs", context)?;
        fs::write(examples_dir.join("basic_usage.rs"), example_rs)?;

        // Generate README
        let readme = self.handlebars.render("rust/README.md", context)?;
        fs::write(docs_dir.join("README.md"), readme)?;

        info!("Generated Rust project structure");
        Ok(())
    }

    /// Generate JavaScript project
    async fn generate_javascript_project(
        &self,
        output_dir: &Path,
        context: &HashMap<&str, &str>,
    ) -> Result<()> {
        // Create directory structure
        let src_dir = output_dir.join("src");
        let tests_dir = output_dir.join("tests");
        let examples_dir = output_dir.join("examples");

        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&tests_dir)?;
        fs::create_dir_all(&examples_dir)?;

        // Generate package.json
        let package_json = self.handlebars.render("javascript/package.json", context)?;
        fs::write(output_dir.join("package.json"), package_json)?;

        // Generate main implementation
        let index_js = self.handlebars.render("javascript/index.js", context)?;
        fs::write(src_dir.join("index.js"), index_js)?;

        // Generate tests
        let test_js = self.handlebars.render("javascript/test.js", context)?;
        fs::write(tests_dir.join("test.js"), test_js)?;

        // Generate example
        let example_js = self.handlebars.render("javascript/example.js", context)?;
        fs::write(examples_dir.join("example.js"), example_js)?;

        info!("Generated JavaScript project structure");
        Ok(())
    }
}

/// Register Rust templates
fn register_rust_templates(handlebars: &mut Handlebars) -> Result<()> {
    // Cargo.toml template
    handlebars.register_template_string(
        "rust/Cargo.toml",
        r#"[package]
name = "{{plugin_name}}"
version = "0.1.0"
edition = "2021"
description = "A {{plugin_type}} plugin for DataFlare"
authors = ["Your Name <your.email@example.com>"]
license = "MIT"

[workspace]

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
wasm-bindgen = "0.2"
chrono = { version = "0.4", features = ["serde"] }

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
"#,
    )?;

    // lib.rs template
    handlebars.register_template_string(
        "rust/lib.rs",
        r#"//! {{plugin_name}} - A {{plugin_type}} plugin for DataFlare

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    pub id: String,
    pub data: String,
    pub metadata: HashMap<String, String>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingResult {
    Success(DataRecord),
    Error(String),
    Skip,
}

pub fn process_data(input: DataRecord) -> Result<ProcessingResult, String> {
    let data: Value = serde_json::from_str(&input.data)
        .map_err(|e| format!("Failed to parse input data: {}", e))?;

    let processed_data = process_json_data(data)?;

    let output_record = DataRecord {
        id: input.id,
        data: serde_json::to_string(&processed_data)
            .map_err(|e| format!("Failed to serialize output data: {}", e))?,
        metadata: input.metadata,
        created_at: input.created_at,
        updated_at: Some(chrono::Utc::now().timestamp() as u64),
    };

    Ok(ProcessingResult::Success(output_record))
}

fn process_json_data(mut data: Value) -> Result<Value, String> {
    if let Value::Object(ref mut map) = data {
        map.insert(
            "processed_at".to_string(),
            Value::String(chrono::Utc::now().to_rfc3339()),
        );
        map.insert(
            "processed_by".to_string(),
            Value::String("{{plugin_name}}".to_string()),
        );
    }
    Ok(data)
}
"#,
    )?;

    // Tests template
    handlebars.register_template_string(
        "rust/tests.rs",
        r#"//! Integration tests for {{plugin_name}}

use {{crate_name}}::*;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_process_data() {
    let input = DataRecord {
        id: "test-1".to_string(),
        data: json!({"message": "Hello, world!"}).to_string(),
        metadata: HashMap::new(),
        created_at: Some(chrono::Utc::now().timestamp() as u64),
        updated_at: None,
    };

    let result = process_data(input).unwrap();

    match result {
        ProcessingResult::Success(output) => {
            let data: serde_json::Value = serde_json::from_str(&output.data).unwrap();
            assert!(data.get("processed_at").is_some());
            assert_eq!(data.get("processed_by").unwrap(), "{{plugin_name}}");
        }
        _ => panic!("Expected success result"),
    }
}

#[test]
fn test_invalid_json() {
    let input = DataRecord {
        id: "test-invalid".to_string(),
        data: "invalid json".to_string(),
        metadata: HashMap::new(),
        created_at: Some(chrono::Utc::now().timestamp() as u64),
        updated_at: None,
    };

    let result = process_data(input);
    assert!(result.is_err());
}
"#,
    )?;

    // Example template
    handlebars.register_template_string(
        "rust/example.rs",
        r#"//! Example usage of {{plugin_name}}

use {{crate_name}}::*;
use serde_json::json;
use std::collections::HashMap;

fn main() {
    let input = DataRecord {
        id: "example-1".to_string(),
        data: json!({"message": "Hello from example!"}).to_string(),
        metadata: HashMap::new(),
        created_at: Some(chrono::Utc::now().timestamp() as u64),
        updated_at: None,
    };

    match process_data(input) {
        Ok(ProcessingResult::Success(output)) => {
            println!("Processed data: {}", output.data);
        }
        Ok(ProcessingResult::Error(err)) => {
            println!("Processing error: {}", err);
        }
        Ok(ProcessingResult::Skip) => {
            println!("Data was skipped");
        }
        Err(err) => {
            println!("Failed to process: {}", err);
        }
    }
}
"#,
    )?;

    // README template - use a simple version for now
    handlebars.register_template_string(
        "rust/README.md",
        r#"# {{plugin_name}}

A {{plugin_type}} plugin for DataFlare.

## Usage

```rust
use {{plugin_name}}::*;

// Process data
let result = process_data(input_record)?;
```

## Building

```bash
cargo build --target wasm32-wasi --release
```
"#,
    )?;

    Ok(())
}

/// Register JavaScript templates
fn register_javascript_templates(handlebars: &mut Handlebars) -> Result<()> {
    // package.json template
    handlebars.register_template_string(
        "javascript/package.json",
        r#"{
  "name": "{{plugin_name}}",
  "version": "0.1.0",
  "description": "A {{plugin_type}} plugin for DataFlare",
  "main": "src/index.js",
  "scripts": {
    "build": "webpack --mode=production",
    "test": "jest"
  },
  "dependencies": {
    "@dataflare/plugin-sdk": "^1.0.0"
  },
  "devDependencies": {
    "webpack": "^5.0.0",
    "jest": "^29.0.0"
  }
}
"#,
    )?;

    // index.js template
    handlebars.register_template_string(
        "javascript/index.js",
        r#"// {{plugin_name}} - A {{plugin_type}} plugin for DataFlare

export function processData(input) {
    try {
        const data = JSON.parse(input.data);

        // Add processing metadata
        data.processed_at = new Date().toISOString();
        data.processed_by = "{{plugin_name}}";

        return {
            id: input.id,
            data: JSON.stringify(data),
            metadata: input.metadata,
            created_at: input.created_at,
            updated_at: Date.now()
        };
    } catch (error) {
        throw new Error(`Failed to process data: ${error.message}`);
    }
}
"#,
    )?;

    // test.js template
    handlebars.register_template_string(
        "javascript/test.js",
        r#"// Tests for {{plugin_name}}

import { processData } from '../src/index.js';

test('processes data correctly', () => {
    const input = {
        id: 'test-1',
        data: JSON.stringify({ message: 'Hello, world!' }),
        metadata: {},
        created_at: Date.now(),
        updated_at: null
    };

    const result = processData(input);

    expect(result.id).toBe('test-1');
    const data = JSON.parse(result.data);
    expect(data.message).toBe('Hello, world!');
    expect(data.processed_by).toBe('{{plugin_name}}');
    expect(data.processed_at).toBeDefined();
});
"#,
    )?;

    // example.js template
    handlebars.register_template_string(
        "javascript/example.js",
        r#"// Example usage of {{plugin_name}}

import { processData } from './src/index.js';

const input = {
    id: 'example-1',
    data: JSON.stringify({ message: 'Hello from example!' }),
    metadata: {},
    created_at: Date.now(),
    updated_at: null
};

try {
    const result = processData(input);
    console.log('Processed data:', result.data);
} catch (error) {
    console.error('Processing failed:', error.message);
}
"#,
    )?;

    Ok(())
}

/// Handlebars helper for equality comparison
fn eq_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param1 = h.param(0).and_then(|v| v.value().as_str());
    let param2 = h.param(1).and_then(|v| v.value().as_str());

    let result = match (param1, param2) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    };

    out.write(&result.to_string())?;
    Ok(())
}

impl PluginTemplate {
    /// Generate JavaScript project from file templates
    async fn generate_javascript_project_from_files(
        &self,
        output_dir: &Path,
        context: &HashMap<&str, &str>,
    ) -> Result<()> {
        let template_dir = get_template_dir("javascript", "basic")?;
        self.copy_and_render_template_dir(&template_dir, output_dir, context).await
    }

    /// Generate Go project from file templates
    async fn generate_go_project_from_files(
        &self,
        output_dir: &Path,
        context: &HashMap<&str, &str>,
    ) -> Result<()> {
        let template_dir = get_template_dir("go", "basic")?;
        self.copy_and_render_template_dir(&template_dir, output_dir, context).await
    }

    /// Copy and render template directory
    async fn copy_and_render_template_dir(
        &self,
        template_dir: &Path,
        output_dir: &Path,
        context: &HashMap<&str, &str>,
    ) -> Result<()> {
        use walkdir::WalkDir;

        for entry in WalkDir::new(template_dir) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let relative_path = path.strip_prefix(template_dir)?;
                let output_path = output_dir.join(relative_path);

                // Create parent directories
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                // Read template file
                let template_content = fs::read_to_string(path)?;

                // Render template
                let rendered_content = self.handlebars.render_template(&template_content, context)?;

                // Write rendered content
                fs::write(&output_path, rendered_content)?;

                info!("Generated: {}", relative_path.display());
            }
        }

        Ok(())
    }
}

/// Get template directory path
fn get_template_dir(language: &str, template: &str) -> Result<PathBuf> {
    // Try to find templates relative to the CLI binary
    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine executable directory"))?
        .to_path_buf();

    // Look for templates in several possible locations
    let possible_paths = vec![
        exe_dir.join("templates").join(language).join(template),
        PathBuf::from("templates").join(language).join(template),
        PathBuf::from("dataflare/wasm/cli/templates").join(language).join(template),
        PathBuf::from("wasm/cli/templates").join(language).join(template),
        PathBuf::from("cli/templates").join(language).join(template),
    ];

    for path in possible_paths {
        if path.exists() {
            return Ok(path);
        }
    }

    anyhow::bail!("Template directory not found for language: {} template: {}", language, template)
}

/// Register Go templates (fallback if file templates not found)
fn register_go_templates(handlebars: &mut Handlebars) -> Result<()> {
    // go.mod template
    handlebars.register_template_string(
        "go/go.mod",
        r#"module {{plugin_name}}

go 1.21

require (
    github.com/dataflare/plugin-sdk-go v1.0.0
)
"#,
    )?;

    // main.go template
    handlebars.register_template_string(
        "go/main.go",
        r#"// {{plugin_name}} - A {{plugin_type}} plugin for DataFlare
package main

import (
    "encoding/json"
    "fmt"
    "log"
)

// Process function - main plugin logic
func Process(data []byte) ([]byte, error) {
    // TODO: Implement your plugin logic here
    var input map[string]interface{}
    if err := json.Unmarshal(data, &input); err != nil {
        return nil, err
    }

    // Add processing metadata
    input["processed_by"] = "{{plugin_name}}"
    input["processed_at"] = "2024-01-01T00:00:00Z" // Use proper timestamp

    return json.Marshal(input)
}

// Info returns plugin information
func Info() (string, string) {
    return "{{plugin_name}}", "0.1.0"
}

func main() {
    fmt.Println("{{plugin_name}} - DataFlare {{plugin_type}} Plugin")
}
"#,
    )?;

    Ok(())
}