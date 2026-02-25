use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "plugin-scaffold")]
#[command(about = "Scaffold a new Skylet plugin", long_about = None)]
struct Args {
    #[arg(short, long, help = "Plugin name (kebab-case)")]
    name: Option<String>,

    #[arg(short, long, help = "Plugin description")]
    description: Option<String>,

    #[arg(
        short,
        long,
        value_enum,
        default_value = "rust",
        help = "Plugin language"
    )]
    language: Language,

    #[arg(short, long, help = "Output directory")]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
enum Language {
    Rust,
}

fn prompt_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn validate_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        && !name.starts_with('-')
}

fn create_plugin_dir(output_dir: &PathBuf, name: &str) -> Result<PathBuf> {
    let plugin_dir = output_dir.join(name);
    if plugin_dir.exists() {
        anyhow::bail!("Plugin directory already exists: {}", plugin_dir.display());
    }
    fs::create_dir_all(plugin_dir.join("src"))?;
    Ok(plugin_dir)
}

fn generate_cargo_toml(name: &str, description: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "{description}"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
skylet-abi = {{ git = "https://github.com/vincents-ai/skylet-abi.git", branch = "main" }}
serde = {{ workspace = true }}
serde_json = {{ workspace = true }}
anyhow = {{ workspace = true }}
tokio = {{ workspace = true }}
tracing = {{ workspace = true }}

[profile.release]
opt-level = 3
lto = true
"#
    )
}

fn generate_lib_rs(name: &str, description: &str) -> String {
    let name_pascal = to_pascal_case(name);
    let name_snake = name.replace('-', "_");

    let template = r#"use skylet_abi::v2::{Plugin, PluginInfo, PluginInitData, FfiResult};
use std::sync::Arc;

pub struct PLUGIN_PASCAL {
    _name: String,
}

impl PLUGIN_PASCAL {
    pub fn new() -> Self {
        Self {
            _name: "PLUGIN_NAME".to_string(),
        }
    }
}

impl Default for PLUGIN_PASCAL {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PLUGIN_PASCAL {
    fn init(&mut self, _init_data: PluginInitData) -> FfiResult<()> {
        tracing::info!("PLUGIN_NAME plugin initialized");
        Ok(())
    }

    fn shutdown(&mut self) -> FfiResult<()> {
        tracing::info!("PLUGIN_NAME plugin shutdown");
        Ok(())
    }

    fn get_info(&self) -> PluginInfo {
        PluginInfo::new(
            "PLUGIN_NAME",
            env!("CARGO_PKG_VERSION"),
            "PLUGIN_DESCRIPTION",
        )
    }
}

#[no_mangle]
pub unsafe extern "C" fn plugin_init_v2(
    _init_data: PluginInitData,
) -> FfiResult<Arc<dyn Plugin>> {
    tracing::info!("Initializing PLUGIN_NAME plugin");
    Ok(Arc::new(PLUGIN_PASCAL::new()))
}

#[no_mangle]
pub unsafe extern "C" fn plugin_shutdown_v2(_plugin: Arc<dyn Plugin>) -> FfiResult<()> {
    tracing::info!("Shutting down PLUGIN_NAME plugin");
    Ok(())
}

#[no_mangle]
pub unsafe extern "C" fn plugin_get_info_v2(_plugin: Arc<dyn Plugin>) -> PluginInfo {
    PluginInfo::new(
        "PLUGIN_NAME",
        env!("CARGO_PKG_VERSION"),
        "PLUGIN_DESCRIPTION",
    )
}"#;

    template
        .replace("PLUGIN_NAME", name)
        .replace("PLUGIN_PASCAL", &name_pascal)
        .replace("PLUGIN_SNAKE", &name_snake)
        .replace("PLUGIN_DESCRIPTION", description)
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in s.chars() {
        if c == '-' || c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

fn create_gitignore(dir: &PathBuf) -> Result<()> {
    let content = r#"/target
*.so
*.dylib
*.dll
.DS_Store
"#;
    fs::write(dir.join(".gitignore"), content)?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (name, description) = if args.name.is_some() && args.description.is_some() {
        (args.name.unwrap(), args.description.unwrap())
    } else {
        println!("Skylet Plugin Scaffolder");
        println!("========================\n");

        let name = if let Some(n) = args.name {
            n
        } else {
            loop {
                let input = prompt_input("Plugin name (kebab-case): ")?;
                if validate_name(&input) {
                    break input;
                }
                println!("Invalid name. Use kebab-case (e.g., my-awesome-plugin)");
            }
        };

        let description = if let Some(d) = args.description {
            d
        } else {
            prompt_input("Plugin description: ")?
        };

        (name, description)
    };

    if !validate_name(&name) {
        anyhow::bail!("Invalid plugin name: {}", name);
    }

    let output_dir = args.output.unwrap_or_else(|| PathBuf::from("."));
    let plugin_dir = create_plugin_dir(&output_dir, &name)?;

    println!("\nCreating plugin: {}", name);
    println!("Output directory: {}", plugin_dir.display());

    fs::write(
        plugin_dir.join("Cargo.toml"),
        generate_cargo_toml(&name, &description),
    )?;
    fs::write(
        plugin_dir.join("src/lib.rs"),
        generate_lib_rs(&name, &description),
    )?;
    create_gitignore(&plugin_dir)?;

    println!("\nPlugin scaffold created successfully!");
    println!("\nTo build your plugin, run:");
    println!("  cd {}", plugin_dir.display());
    println!("  cargo build --release");

    Ok(())
}
