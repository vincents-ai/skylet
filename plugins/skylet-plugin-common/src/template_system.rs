// Plugin template generator and scaffolding system for skylet-plugin-common v0.3.0
use crate::PluginCommonError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Plugin category
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginCategory {
    #[serde(rename = "api-integration")]
    ApiIntegration,
    #[serde(rename = "database")]
    Database,
    #[serde(rename = "ai-workflow")]
    AiWorkflow,
    #[serde(rename = "communication")]
    Communication,
    #[serde(rename = "devops")]
    DevOps,
    #[serde(rename = "infrastructure")]
    Infrastructure,
}

impl PluginCategory {
    /// Get all available categories
    pub fn all() -> Vec<&'static str> {
        vec![
            "api-integration",
            "database",
            "ai-workflow",
            "communication",
            "devops",
            "infrastructure",
        ]
    }

    /// Get category display name
    pub fn display_name(&self) -> &'static str {
        match self {
            PluginCategory::ApiIntegration => "API Integration",
            PluginCategory::Database => "Database",
            PluginCategory::AiWorkflow => "AI & Workflow",
            PluginCategory::Communication => "Communication",
            PluginCategory::DevOps => "DevOps",
            PluginCategory::Infrastructure => "Infrastructure",
        }
    }
}

/// Plugin template definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTemplate {
    pub name: String,
    pub description: String,
    pub category: PluginCategory,
    pub dependencies: Vec<String>,
    pub scaffolding: ScaffoldingConfig,
    pub example_usage: Option<String>,
    pub features: Vec<String>,
}

/// Scaffolding configuration for template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldingConfig {
    pub files: Vec<TemplateFile>,
    pub variables: Vec<TemplateVariable>,
    pub post_generation: Vec<PostGenerationStep>,
}

/// Template file definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    pub path: String,
    pub template: String,
    pub file_type: FileType,
}

/// File type for template files
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileType {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "config")]
    Config,
    #[serde(rename = "documentation")]
    Documentation,
    #[serde(rename = "test")]
    Test,
    #[serde(rename = "build")]
    Build,
    #[serde(rename = "executable")]
    Executable,
}

/// Template variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub description: String,
    pub variable_type: VariableType,
    pub required: bool,
    pub default_value: Option<String>,
    pub validation: Option<VariableValidation>,
}

/// Variable type for template variables
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VariableType {
    #[serde(rename = "string")]
    String,
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "boolean")]
    Boolean,
    #[serde(rename = "select")]
    Select { options: Vec<String> },
    #[serde(rename = "multiselect")]
    MultiSelect { options: Vec<String> },
    #[serde(rename = "file")]
    File,
    #[serde(rename = "directory")]
    Directory,
}

/// Validation rules for variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableValidation {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
}

/// Post-generation step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostGenerationStep {
    pub step_type: PostGenerationType,
    pub description: String,
    pub command: Option<String>,
    pub condition: Option<String>,
}

/// Type of post-generation step
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PostGenerationType {
    #[serde(rename = "cargo-fmt")]
    CargoFmt,
    #[serde(rename = "cargo-clippy")]
    CargoClippy,
    #[serde(rename = "cargo-test")]
    CargoTest,
    #[serde(rename = "cargo-build")]
    CargoBuild,
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "git-init")]
    GitInit,
    #[serde(rename = "git-add")]
    GitAdd,
    #[serde(rename = "git-commit")]
    GitCommit,
}

/// Plugin generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub template_name: String,
    pub output_directory: String,
    pub variables: HashMap<String, String>,
    pub overwrite_existing: bool,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            template_name: "basic-api".to_string(),
            output_directory: "./my-plugin".to_string(),
            variables: HashMap::new(),
            overwrite_existing: false,
        }
    }
}

/// Generated plugin result
#[derive(Debug, Clone)]
pub struct GeneratedPlugin {
    pub name: String,
    pub directory: String,
    pub files_created: Vec<String>,
    pub post_generation_output: Vec<String>,
}

/// Template generator for plugin scaffolding
pub struct TemplateGenerator {
    templates: HashMap<String, PluginTemplate>,
    template_directory: Option<String>,
}

impl TemplateGenerator {
    /// Create a new template generator
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_directory: None,
        }
    }

    /// Set template directory
    pub fn with_template_directory(mut self, directory: &str) -> Self {
        self.template_directory = Some(directory.to_string());
        self
    }

    /// Load templates from directory
    pub fn load_templates_from_directory(
        &mut self,
        directory: &str,
    ) -> Result<(), PluginCommonError> {
        let template_dir = Path::new(directory);
        if !template_dir.exists() {
            return Err(PluginCommonError::SerializationFailed(format!(
                "Template directory does not exist: {}",
                directory
            )));
        }

        // Load template files (simplified - in real implementation would scan for .toml files)
        let api_integration_template = self.create_api_integration_template()?;
        let database_template = self.create_database_template()?;
        let communication_template = self.create_communication_template()?;

        self.templates
            .insert("api-integration".to_string(), api_integration_template);
        self.templates
            .insert("database".to_string(), database_template);
        self.templates
            .insert("communication".to_string(), communication_template);

        Ok(())
    }

    /// Generate a plugin from template
    pub fn generate_plugin(
        &self,
        config: &GenerationConfig,
    ) -> Result<GeneratedPlugin, PluginCommonError> {
        let template = self.templates.get(&config.template_name).ok_or_else(|| {
            PluginCommonError::SerializationFailed(format!(
                "Template '{}' not found",
                config.template_name
            ))
        })?;

        // Create output directory
        fs::create_dir_all(&config.output_directory).map_err(|e| {
            PluginCommonError::SerializationFailed(format!(
                "Failed to create output directory: {}",
                e
            ))
        })?;

        let mut files_created = Vec::new();

        // Process each template file
        for template_file in &template.scaffolding.files {
            let processed_content =
                self.process_template(&template_file.template, &config.variables)?;

            // Security: Validate path to prevent directory traversal attacks
            let output_base = std::fs::canonicalize(&config.output_directory).map_err(|e| {
                PluginCommonError::SerializationFailed(format!(
                    "Failed to resolve output directory: {}",
                    e
                ))
            })?;

            let file_path = output_base.join(&template_file.path);

            // Ensure the file path is within the output directory
            match file_path.canonicalize() {
                Ok(canonical_path) => {
                    if !canonical_path.starts_with(&output_base) {
                        return Err(PluginCommonError::SerializationFailed(format!(
                            "Path traversal detected: {} is outside output directory",
                            template_file.path
                        )));
                    }
                }
                Err(_) => {
                    // File doesn't exist yet, validate parent directory instead
                    if let Some(parent) = file_path.parent() {
                        if !parent.starts_with(&output_base) {
                            return Err(PluginCommonError::SerializationFailed(format!(
                                "Path traversal detected: {} is outside output directory",
                                template_file.path
                            )));
                        }
                    }
                }
            }

            // Create parent directories if needed
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    PluginCommonError::SerializationFailed(format!(
                        "Failed to create directory: {}",
                        e
                    ))
                })?;
            }

            fs::write(&file_path, processed_content).map_err(|e| {
                PluginCommonError::SerializationFailed(format!(
                    "Failed to write file {}: {}",
                    file_path.display(),
                    e
                ))
            })?;

            files_created.push(file_path.to_string_lossy().to_string());
        }

        let mut post_generation_output = Vec::new();

        // Run post-generation steps
        for step in &template.scaffolding.post_generation {
            match step.step_type {
                PostGenerationType::CargoFmt => {
                    // Run cargo fmt
                    if let Ok(output) =
                        self.run_command("cargo", &["fmt"], &config.output_directory)
                    {
                        post_generation_output.push(output);
                    }
                }
                PostGenerationType::CargoClippy => {
                    // Run cargo clippy
                    if let Ok(output) =
                        self.run_command("cargo", &["clippy"], &config.output_directory)
                    {
                        post_generation_output.push(output);
                    }
                }
                PostGenerationType::CargoTest => {
                    // Run cargo test
                    if let Ok(output) =
                        self.run_command("cargo", &["test"], &config.output_directory)
                    {
                        post_generation_output.push(output);
                    }
                }
                PostGenerationType::GitInit => {
                    // Initialize git repository
                    if let Ok(output) = self.run_command("git", &["init"], &config.output_directory)
                    {
                        post_generation_output.push(output);
                    }
                }
                _ => {
                    post_generation_output.push(format!("Skipped step: {:?}", step.step_type));
                }
            }
        }

        Ok(GeneratedPlugin {
            name: template.name.clone(),
            directory: config.output_directory.clone(),
            files_created,
            post_generation_output,
        })
    }

    /// List available templates
    pub fn list_templates(&self, category: Option<PluginCategory>) -> Vec<&PluginTemplate> {
        if let Some(cat) = category {
            self.templates
                .values()
                .filter(|t| t.category == cat)
                .collect()
        } else {
            self.templates.values().collect()
        }
    }

    /// Get template by name
    pub fn get_template(&self, name: &str) -> Option<&PluginTemplate> {
        self.templates.get(name)
    }

    /// Process template content with variables
    fn process_template(
        &self,
        template: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String, PluginCommonError> {
        let mut result = template.to_string();

        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }

        Ok(result)
    }

    /// Run a command in the specified directory
    fn run_command(
        &self,
        command: &str,
        args: &[&str],
        directory: &str,
    ) -> Result<String, PluginCommonError> {
        use std::process::Command;

        let output = Command::new(command)
            .args(args)
            .current_dir(directory)
            .output()
            .map_err(|e| {
                PluginCommonError::SerializationFailed(format!(
                    "Failed to run command {}: {}",
                    command, e
                ))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{} exited successfully", command))
        } else {
            Ok(format!("{} failed: {}", command, stderr))
        }
    }

    /// Create API integration template
    fn create_api_integration_template(&self) -> Result<PluginTemplate, PluginCommonError> {
        Ok(PluginTemplate {
            name: "API Integration Plugin".to_string(),
            description: "Plugin for integrating with external APIs".to_string(),
            category: PluginCategory::ApiIntegration,
            dependencies: vec![
                "skylet-plugin-common".to_string(),
                "serde".to_string(),
                "serde_json".to_string(),
                "ureq".to_string(),
            ],
            scaffolding: ScaffoldingConfig {
                files: vec![
                    TemplateFile {
                        path: "Cargo.toml".to_string(),
                        template: self.get_cargo_template("api-integration"),
                        file_type: FileType::Config,
                    },
                    TemplateFile {
                        path: "src/lib.rs".to_string(),
                        template: self.get_api_integration_source_template(),
                        file_type: FileType::Source,
                    },
                    TemplateFile {
                        path: "README.md".to_string(),
                        template: self.get_readme_template("API Integration Plugin"),
                        file_type: FileType::Documentation,
                    },
                ],
                variables: vec![
                    TemplateVariable {
                        name: "plugin_name".to_string(),
                        description: "Name of the plugin".to_string(),
                        variable_type: VariableType::String,
                        required: true,
                        default_value: Some("my-api-plugin".to_string()),
                        validation: None,
                    },
                    TemplateVariable {
                        name: "api_base_url".to_string(),
                        description: "Base URL for the API".to_string(),
                        variable_type: VariableType::String,
                        required: true,
                        default_value: Some("https://api.example.com".to_string()),
                        validation: None,
                    },
                    TemplateVariable {
                        name: "auth_type".to_string(),
                        description: "Authentication type".to_string(),
                        variable_type: VariableType::Select {
                            options: vec![
                                "bearer".to_string(),
                                "api-key".to_string(),
                                "basic".to_string(),
                            ],
                        },
                        required: false,
                        default_value: Some("bearer".to_string()),
                        validation: None,
                    },
                ],
                post_generation: vec![
                    PostGenerationStep {
                        step_type: PostGenerationType::CargoFmt,
                        description: "Format the code".to_string(),
                        command: None,
                        condition: None,
                    },
                    PostGenerationStep {
                        step_type: PostGenerationType::CargoTest,
                        description: "Run tests".to_string(),
                        command: None,
                        condition: None,
                    },
                ],
            },
            example_usage: Some(
                "skylet-plugin generate --template api-integration --name my-plugin --api-base-url https://api.example.com".to_string()
            ),
            features: vec![
                "HTTP client with authentication".to_string(),
                "Rate limiting".to_string(),
                "Error handling".to_string(),
                "JSON response parsing".to_string(),
            ],
        })
    }

    /// Create database template
    fn create_database_template(&self) -> Result<PluginTemplate, PluginCommonError> {
        Ok(PluginTemplate {
            name: "Database Plugin".to_string(),
            description: "Plugin for database operations".to_string(),
            category: PluginCategory::Database,
            dependencies: vec![
                "skylet-plugin-common".to_string(),
                "sqlx".to_string(),
                "serde".to_string(),
            ],
            scaffolding: ScaffoldingConfig {
                files: vec![
                    TemplateFile {
                        path: "Cargo.toml".to_string(),
                        template: self.get_cargo_template("database"),
                        file_type: FileType::Config,
                    },
                    TemplateFile {
                        path: "src/lib.rs".to_string(),
                        template: self.get_database_source_template(),
                        file_type: FileType::Source,
                    },
                ],
                variables: vec![
                    TemplateVariable {
                        name: "plugin_name".to_string(),
                        description: "Name of the plugin".to_string(),
                        variable_type: VariableType::String,
                        required: true,
                        default_value: Some("my-database-plugin".to_string()),
                        validation: None,
                    },
                    TemplateVariable {
                        name: "database_type".to_string(),
                        description: "Database type".to_string(),
                        variable_type: VariableType::Select {
                            options: vec![
                                "postgresql".to_string(),
                                "mysql".to_string(),
                                "sqlite".to_string(),
                            ],
                        },
                        required: true,
                        default_value: Some("postgresql".to_string()),
                        validation: None,
                    },
                ],
                post_generation: vec![
                    PostGenerationStep {
                        step_type: PostGenerationType::CargoFmt,
                        description: "Format the code".to_string(),
                        command: None,
                        condition: None,
                    },
                ],
            },
            example_usage: Some(
                "skylet-plugin generate --template database --name my-db-plugin --database-type postgresql".to_string()
            ),
            features: vec![
                "Connection pooling".to_string(),
                "Query builder".to_string(),
                "Transaction support".to_string(),
                "Error handling".to_string(),
            ],
        })
    }

    /// Create communication template
    fn create_communication_template(&self) -> Result<PluginTemplate, PluginCommonError> {
        Ok(PluginTemplate {
            name: "Communication Plugin".to_string(),
            description: "Plugin for communication platforms".to_string(),
            category: PluginCategory::Communication,
            dependencies: vec![
                "skylet-plugin-common".to_string(),
                "tokio".to_string(),
                "serde".to_string(),
            ],
            scaffolding: ScaffoldingConfig {
                files: vec![
                    TemplateFile {
                        path: "Cargo.toml".to_string(),
                        template: self.get_cargo_template("communication"),
                        file_type: FileType::Config,
                    },
                    TemplateFile {
                        path: "src/lib.rs".to_string(),
                        template: self.get_communication_source_template(),
                        file_type: FileType::Source,
                    },
                ],
                variables: vec![
                    TemplateVariable {
                        name: "plugin_name".to_string(),
                        description: "Name of the plugin".to_string(),
                        variable_type: VariableType::String,
                        required: true,
                        default_value: Some("my-communication-plugin".to_string()),
                        validation: None,
                    },
                    TemplateVariable {
                        name: "platform".to_string(),
                        description: "Communication platform".to_string(),
                        variable_type: VariableType::Select {
                            options: vec![
                                "telegram".to_string(),
                                "discord".to_string(),
                                "slack".to_string(),
                            ],
                        },
                        required: true,
                        default_value: Some("telegram".to_string()),
                        validation: None,
                    },
                ],
                post_generation: vec![
                    PostGenerationStep {
                        step_type: PostGenerationType::CargoFmt,
                        description: "Format the code".to_string(),
                        command: None,
                        condition: None,
                    },
                ],
            },
            example_usage: Some(
                "skylet-plugin generate --template communication --name my-telegram-plugin --platform telegram".to_string()
            ),
            features: vec![
                "Message handling".to_string(),
                "Command registration".to_string(),
                "User management".to_string(),
                "Webhook support".to_string(),
            ],
        })
    }

    /// Get Cargo.toml template for different plugin types
    fn get_cargo_template(&self, plugin_type: &str) -> String {
        match plugin_type {
            "api-integration" => r#"[package]
name = "{{plugin_name}}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-plugin-common = { path = "../skylet-plugin-common" }
marketplace-abi = { path = "../../abi" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
ureq = "2.8"
anyhow = "1.0"
"#
            .to_string(),
            "database" => r#"[package]
name = "{{plugin_name}}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-plugin-common = { path = "../skylet-plugin-common" }
marketplace-abi = { path = "../../abi" }
serde = { version = "1.0", features = ["derive"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres"] }
anyhow = "1.0"
"#
            .to_string(),
            "communication" => r#"[package]
name = "{{plugin_name}}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-plugin-common = { path = "../skylet-plugin-common" }
marketplace-abi = { path = "../../abi" }
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["rt", "macros"] }
anyhow = "1.0"
"#
            .to_string(),
            _ => r#"[package]
name = "{{plugin_name}}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-plugin-common = { path = "../skylet-plugin-common" }
marketplace-abi = { path = "../../abi" }
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"
"#
            .to_string(),
        }
    }

    /// Get API integration source template
    fn get_api_integration_source_template(&self) -> String {
        r#"use skylet_plugin_common::*;
use serde_json::json;

skylet_plugin! {
    name: "{{plugin_name}}",
    version: "0.1.0",
    description: "API integration plugin for {{api_base_url}}",
    plugin_type: PluginType::Integration,
    max_concurrency: 10,
}

#[no_mangle]
pub extern "C" fn handle_action(
    _context: *const skylet_abi::PluginContext,
    args_json: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    handle_json_request(|args| {
        let endpoint: String = args["endpoint"].as_str().unwrap_or("").to_string();
        let base_url = "{{api_base_url}}";
        
        let client = create_authenticated_client(
            &base_url,
            AuthConfig::Bearer { token: args["token"].as_str().unwrap_or("") }
        )?;
        
        let result: serde_json::Value = client
            .get_with_auth(&endpoint)?;
            
        Ok(json!({
            "data": result,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }, args_json)
}
"#
        .to_string()
    }

    /// Get database source template
    fn get_database_source_template(&self) -> String {
        r#"use skylet_plugin_common::*;
use serde_json::json;

skylet_plugin! {
    name: "{{plugin_name}}",
    version: "0.1.0",
    description: "{{database_type}} database plugin",
    plugin_type: PluginType::Database,
    max_concurrency: 10,
}

#[no_mangle]
pub extern "C" fn handle_action(
    _context: *const skylet_abi::PluginContext,
    args_json: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    handle_json_request(|args| {
        let query: String = args["query"].as_str().unwrap_or("").to_string();
        
        // Create database config
        let config = DatabaseConfig::new("connection_string");
        
        // Use query builder (simplified example)
        let (sql, _params) = select("users")
            .where_eq("id", 42i64)
            .order_by_asc("name")
            .limit(10)
            .build()
            .unwrap();
            
        Ok(json!({
            "query": sql,
            "success": true
        }))
    }, args_json)
}
"#
        .to_string()
    }

    /// Get communication source template
    fn get_communication_source_template(&self) -> String {
        r#"use skylet_plugin_common::*;
use serde_json::json;

skylet_plugin! {
    name: "{{plugin_name}}",
    version: "0.1.0",
    description: "{{platform}} communication plugin",
    plugin_type: PluginType::Integration,
    max_concurrency: 10,
}

#[no_mangle]
pub extern "C" fn handle_action(
    _context: *const skylet_abi::PluginContext,
    args_json: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    handle_json_request(|args| {
        let message: String = args["message"].as_str().unwrap_or("").to_string();
        let chat_id: String = args["chat_id"].as_str().unwrap_or("").to_string();
        
        // Handle message based on platform
        match "{{platform}}" {
            "telegram" => {
                // Telegram-specific logic
                Ok(json!({
                    "message": format!("Received message: {}", message),
                    "chat_id": chat_id
                }))
            }
            _ => {
                Ok(json!({
                    "error": "Unsupported platform",
                    "platform": "{{platform}}"
                }))
            }
        }
    }, args_json)
}
"#
        .to_string()
    }

    /// Get README template
    fn get_readme_template(&self, plugin_type: &str) -> String {
        format!(
            r#"# {{plugin_name}}

{} plugin built with skylet-plugin-common v0.3.0.

## Features

- Built-in authentication support
- Rate limiting
- Error handling
- JSON serialization/deserialization
- Plugin metadata management

## Usage

```rust
use skylet_plugin_common::*;

// Plugin is automatically registered with the Skylet ecosystem
```

## Configuration

Configure your plugin settings through environment variables or config files.

## Development

```bash
cargo build
cargo test
```

## License

MIT
"#,
            plugin_type
        )
    }
}

impl Default for TemplateGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Code generation engine for template processing
pub struct CodeGenerator {
    templates: HashMap<String, Template>,
}

/// Simple template representation
#[derive(Debug, Clone)]
pub struct Template {
    pub content: String,
    pub delimiters: (String, String),
}

impl CodeGenerator {
    /// Create a new code generator
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Register a template
    pub fn register_template(&mut self, name: String, template: Template) {
        self.templates.insert(name, template);
    }

    /// Generate code from template
    pub fn generate(
        &self,
        template_name: &str,
        context: &serde_json::Value,
    ) -> Result<String, PluginCommonError> {
        let template = self.templates.get(template_name).ok_or_else(|| {
            PluginCommonError::SerializationFailed(format!(
                "Template '{}' not found",
                template_name
            ))
        })?;

        let template_file = TemplateFile {
            path: template_name.to_string(),
            template: template.content.clone(),
            file_type: FileType::Source,
        };

        self.process_template(&template_file, context)
    }

    /// Generate a file from template
    pub fn generate_file(
        &self,
        template_file: &TemplateFile,
        context: &serde_json::Value,
    ) -> Result<GeneratedFile, PluginCommonError> {
        let content = self.process_template(
            &TemplateFile {
                path: template_file.path.clone(),
                template: template_file.template.clone(),
                file_type: template_file.file_type.clone(),
            },
            context,
        )?;

        Ok(GeneratedFile {
            path: template_file.path.clone(),
            content,
            file_type: template_file.file_type.clone(),
        })
    }

    /// Process template with context
    fn process_template(
        &self,
        template: &TemplateFile,
        context: &serde_json::Value,
    ) -> Result<String, PluginCommonError> {
        let mut content = template.template.clone();

        // Simple variable replacement (in real implementation would use a proper templating engine)
        if let serde_json::Value::Object(map) = context {
            for (key, value) in map {
                if let serde_json::Value::String(s) = value {
                    let placeholder = format!("{{{{{}}}}}", key);
                    content = content.replace(&placeholder, s);
                }
            }
        }

        Ok(content)
    }
}

/// Generated file information
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
    pub file_type: FileType,
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions
pub fn create_template_generator() -> TemplateGenerator {
    TemplateGenerator::new()
}

pub fn create_code_generator() -> CodeGenerator {
    CodeGenerator::new()
}

/// Create a generation config with sensible defaults
pub fn create_generation_config(template_name: &str, output_dir: &str) -> GenerationConfig {
    GenerationConfig {
        template_name: template_name.to_string(),
        output_directory: output_dir.to_string(),
        variables: HashMap::new(),
        overwrite_existing: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_template_creation() {
        let generator = TemplateGenerator::new();
        let template = generator.create_api_integration_template().unwrap();

        assert_eq!(template.name, "API Integration Plugin");
        assert_eq!(template.category, PluginCategory::ApiIntegration);
        assert!(template.dependencies.contains(&"serde".to_string()));
    }

    #[test]
    fn test_template_variable_processing() {
        let generator = TemplateGenerator::new();
        let mut variables = HashMap::new();
        variables.insert("plugin_name".to_string(), "my-test-plugin".to_string());
        variables.insert(
            "api_base_url".to_string(),
            "https://test.api.com".to_string(),
        );

        let template_content = r#"Plugin: {{plugin_name}} for {{api_base_url}}"#;
        let processed = generator
            .process_template(template_content, &variables)
            .unwrap();

        assert!(processed.contains("my-test-plugin"));
        assert!(processed.contains("https://test.api.com"));
    }

    #[test]
    fn test_generation_config() {
        let config = GenerationConfig::default();
        assert_eq!(config.template_name, "basic-api");
        assert_eq!(config.output_directory, "./my-plugin");
        assert_eq!(config.overwrite_existing, false);
    }

    #[test]
    fn test_plugin_category_display_names() {
        assert_eq!(
            PluginCategory::ApiIntegration.display_name(),
            "API Integration"
        );
        assert_eq!(PluginCategory::Database.display_name(), "Database");
        assert_eq!(
            PluginCategory::Communication.display_name(),
            "Communication"
        );
    }
}
