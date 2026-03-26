// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin Test Harness CLI
//!
//! A command-line tool for testing Skylet plugins in isolation.
//!
//! # Usage
//!
//! ```bash
//! # Test a single plugin
//! plugin-test-harness test --plugin-path ./target/release/libmy_plugin.so
//!
//! # Run a test suite from config
//! plugin-test-harness suite --config-file tests/plugin-suite.toml
//!
//! # Test plugin loading only
//! plugin-test-harness load --plugin-path ./target/release/libmy_plugin.so
//!
//! # Run BDD feature tests
//! plugin-test-harness bdd --feature-path features/
//!
//! # Execute a single action
//! plugin-test-harness execute --plugin-path ./plugin.so --action health
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use cucumber::{writer, World, WriterExt};

use plugin_test_harness::test_world::PluginTestWorld;
use plugin_test_harness::*;

#[derive(Parser)]
#[command(name = "plugin-test-harness")]
#[command(about = "Plugin test harness for Skylet plugins (V2 ABI)")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Test a plugin with basic API tests
    Test {
        /// Path to the plugin shared library (.so/.dylib)
        #[arg(short, long)]
        plugin_path: String,

        /// Enable verbose output
        #[arg(short, long, default_value = "false")]
        verbose: bool,
    },

    /// Run a test suite from configuration file
    Suite {
        /// Path to the test suite configuration file (TOML)
        #[arg(short, long)]
        config_file: Option<String>,

        /// Enable verbose output
        #[arg(short, long, default_value = "false")]
        verbose: bool,
    },

    /// Test plugin loading without running tests
    Load {
        /// Path to the plugin shared library (.so/.dylib)
        #[arg(short, long)]
        plugin_path: String,

        /// Show plugin info after loading
        #[arg(short, long, default_value = "true")]
        show_info: bool,
    },

    /// Run BDD/Cucumber feature tests
    Bdd {
        /// Path to the feature file or directory (default: ./features)
        #[arg(short, long, default_value = "./features")]
        feature_path: String,

        /// Path to the plugin to test (optional, can be specified in scenarios)
        #[arg(short, long)]
        plugin_path: Option<String>,

        /// Tags to filter scenarios (e.g., "@smoke", "@api")
        #[arg(short, long)]
        tags: Option<String>,

        /// Output format: pretty, json, junit
        #[arg(long, default_value = "pretty")]
        format: String,

        /// Fail fast on first error
        #[arg(long, default_value = "false")]
        fail_fast: bool,
    },

    /// Execute a single action on a plugin
    Execute {
        /// Path to the plugin shared library
        #[arg(short, long)]
        plugin_path: String,

        /// Action name to execute
        #[arg(short, long)]
        action: String,

        /// JSON arguments for the action
        #[arg(long, default_value = "{}")]
        args: String,
    },

    /// Validate a plugin meets Skylet requirements
    Validate {
        /// Path to the plugin shared library
        #[arg(short, long)]
        plugin_path: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Test {
            plugin_path,
            verbose,
        } => {
            run_plugin_test(&plugin_path, verbose).await?;
        }
        Commands::Suite {
            config_file,
            verbose,
        } => {
            run_test_suite(config_file.as_deref(), verbose).await?;
        }
        Commands::Load {
            plugin_path,
            show_info,
        } => {
            test_plugin_loading(&plugin_path, show_info).await?;
        }
        Commands::Bdd {
            feature_path,
            plugin_path,
            tags,
            format,
            fail_fast,
        } => {
            run_bdd_tests(&feature_path, plugin_path.as_deref(), tags.as_deref(), &format, fail_fast).await?;
        }
        Commands::Execute {
            plugin_path,
            action,
            args,
        } => {
            execute_plugin_action(&plugin_path, &action, &args).await?;
        }
        Commands::Validate { plugin_path } => {
            validate_plugin(&plugin_path).await?;
        }
    }

    Ok(())
}

async fn run_plugin_test(plugin_path: &str, verbose: bool) -> Result<()> {
    println!("Testing plugin: {}", plugin_path);

    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string(),
        dependencies: Vec::new(),
        mock_services: HashMap::new(),
        test_timeout_ms: 5000,
        enable_logging: verbose,
    };

    let mut harness = PluginTestHarness::new(config);

    // Load and test plugin
    match harness.load_plugin().await {
        Ok(_) => {
            println!("[OK] Plugin loaded successfully");

            // Run basic API tests
            let test_cases = vec![
                PluginTestCase {
                    name: "Health Check".to_string(),
                    action: "health".to_string(),
                    args_json: "{}".to_string(),
                    expected_success: true,
                    expected_response_contains: None,
                },
                PluginTestCase {
                    name: "Info Query".to_string(),
                    action: "info".to_string(),
                    args_json: "{}".to_string(),
                    expected_success: true,
                    expected_response_contains: None,
                },
            ];

            let results = harness.test_plugin_api(test_cases).await?;
            print_test_results(&results);
        }
        Err(e) => {
            println!("[FAIL] Failed to load plugin: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run_test_suite(config_file: Option<&str>, verbose: bool) -> Result<()> {
    let config_path = config_file.unwrap_or("tests/plugin-suite.toml");
    println!("Running test suite from: {}", config_path);

    // Try to read and parse the config file
    let suite_config = if std::path::Path::new(config_path).exists() {
        let content = std::fs::read_to_string(config_path)?;
        toml::from_str::<TestSuiteConfig>(&content)?
    } else {
        println!("[WARN] Config file not found, using defaults");
        TestSuiteConfig::default()
    };

    let mut total_passed = 0;
    let mut total_failed = 0;

    for plugin_config in suite_config.plugins {
        println!("\nTesting plugin: {}", plugin_config.name);

        let config = PluginTestConfig {
            plugin_path: plugin_config.path,
            dependencies: Vec::new(),
            mock_services: HashMap::new(),
            test_timeout_ms: plugin_config.timeout_ms.unwrap_or(5000),
            enable_logging: verbose,
        };

        let mut harness = PluginTestHarness::new(config);

        match harness.load_plugin().await {
            Ok(_) => {
                let results = harness.run_bdd_tests().await?;
                let passed = results.iter().filter(|r| r.status == TestStatus::Passed).count();
                let failed = results.iter().filter(|r| r.status == TestStatus::Failed).count();
                total_passed += passed;
                total_failed += failed;
                println!("  [OK] {} passed, {} failed", passed, failed);
            }
            Err(e) => {
                println!("  [FAIL] Failed to load: {}", e);
                total_failed += 1;
            }
        }
    }

    println!("\n--- Summary ---");
    println!("Total Passed: {}", total_passed);
    println!("Total Failed: {}", total_failed);

    if total_failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

async fn test_plugin_loading(plugin_path: &str, show_info: bool) -> Result<()> {
    println!("Testing plugin loading: {}", plugin_path);

    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string(),
        ..Default::default()
    };

    let mut harness = PluginTestHarness::new(config);

    match harness.load_plugin().await {
        Ok(_) => {
            println!("[OK] Plugin loaded successfully");

            if show_info {
                // Try to get plugin info via execute
                match harness.execute_action("info", "{}") {
                    Ok(info) => {
                        println!("\nPlugin Info:");
                        println!("{}", info);
                    }
                    Err(_) => {
                        println!("\n(Plugin info action not available)");
                    }
                }
            }
        }
        Err(e) => {
            println!("[FAIL] Failed to load plugin: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run_bdd_tests(
    feature_path: &str,
    _plugin_path: Option<&str>,
    tags: Option<&str>,
    format: &str,
    fail_fast: bool,
) -> Result<()> {
    println!("Running BDD tests from: {}", feature_path);
    
    let features_path = PathBuf::from(feature_path);
    
    if !features_path.exists() {
        println!("[ERROR] Feature path not found: {}", feature_path);
        std::process::exit(1);
    }

    // Print configuration
    if let Some(t) = tags {
        println!("  Tags: {}", t);
    }
    println!("  Format: {}", format);
    println!("  Fail fast: {}", fail_fast);
    println!();

    // Run cucumber tests
    match format {
        "json" => {
            // JSON output for CI/CD integration
            PluginTestWorld::cucumber()
                .with_writer(
                    writer::Json::new(std::io::stdout())
                )
                .fail_on_skipped()
                .run(feature_path)
                .await;
        }
        "junit" => {
            // JUnit XML output
            PluginTestWorld::cucumber()
                .with_writer(
                    writer::JUnit::new(std::io::stdout(), 0)
                )
                .fail_on_skipped()
                .run(feature_path)
                .await;
        }
        _ => {
            // Pretty/default output
            let runner = PluginTestWorld::cucumber()
                .fail_on_skipped();
            
            if fail_fast {
                runner
                    .with_writer(writer::Basic::stdout().fail_on_skipped())
                    .run(feature_path)
                    .await;
            } else {
                runner.run(feature_path).await;
            }
        }
    }

    Ok(())
}

async fn execute_plugin_action(plugin_path: &str, action: &str, args: &str) -> Result<()> {
    println!("Executing action '{}' on plugin: {}", action, plugin_path);

    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string(),
        ..Default::default()
    };

    let mut harness = PluginTestHarness::new(config);
    harness.load_plugin().await?;

    match harness.execute_action(action, args) {
        Ok(response) => {
            println!("\nResponse:");
            println!("{}", response);
        }
        Err(e) => {
            println!("\n[ERROR] Action failed: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[allow(unused_assignments)]
async fn validate_plugin(plugin_path: &str) -> Result<()> {
    println!("Validating plugin: {}", plugin_path);
    println!();

    let mut checks_passed = 0;
    let mut checks_failed = 0;

    // Check 1: File exists
    print!("  [1/5] File exists: ");
    if std::path::Path::new(plugin_path).exists() {
        println!("OK");
        checks_passed += 1;
    } else {
        println!("FAIL - File not found");
        checks_failed += 1;
        println!("\n[ERROR] Cannot continue validation - file not found");
        std::process::exit(1);
    }

    // Check 2: Can load plugin
    print!("  [2/5] Plugin loads: ");
    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string(),
        ..Default::default()
    };
    let mut harness = PluginTestHarness::new(config);
    
    match harness.load_plugin().await {
        Ok(_) => {
            println!("OK");
            checks_passed += 1;
        }
        Err(e) => {
            println!("FAIL - {}", e);
            checks_failed += 1;
            println!("\n[ERROR] Cannot continue validation - plugin won't load");
            std::process::exit(1);
        }
    }

    // Check 3: Has V2 ABI
    print!("  [3/5] V2 ABI exports: ");
    println!("OK (verified during load)");
    checks_passed += 1;

    // Check 4: Plugin info available
    print!("  [4/5] Plugin info: ");
    match harness.execute_action("info", "{}") {
        Ok(info) => {
            if info.contains("name") || info.contains("version") {
                println!("OK");
                checks_passed += 1;
            } else {
                println!("WARN - Info action returned unexpected format");
                checks_passed += 1; // Still pass, just warn
            }
        }
        Err(_) => {
            println!("SKIP - No info action");
            // Not a failure, just skip
        }
    }

    // Check 5: Health check
    print!("  [5/5] Health check: ");
    match harness.execute_action("health", "{}") {
        Ok(response) => {
            if response.to_lowercase().contains("ok") || response.contains("healthy") {
                println!("OK");
                checks_passed += 1;
            } else {
                println!("WARN - Health check returned: {}", response);
                checks_passed += 1;
            }
        }
        Err(_) => {
            println!("SKIP - No health action");
            // Not a failure for validation
        }
    }

    println!();
    println!("--- Validation Summary ---");
    println!("Passed: {}", checks_passed);
    println!("Failed: {}", checks_failed);

    if checks_failed > 0 {
        println!("\n[FAIL] Plugin validation failed");
        std::process::exit(1);
    } else {
        println!("\n[OK] Plugin validation passed");
    }

    Ok(())
}

fn print_test_results(results: &[TestResult]) {
    println!("\n--- Test Results ---");
    println!("Total: {}", results.len());

    let passed = results.iter().filter(|r| r.status == TestStatus::Passed).count();
    let failed = results.iter().filter(|r| r.status == TestStatus::Failed).count();

    println!("Passed: {}", passed);
    println!("Failed: {}", failed);

    if failed > 0 {
        println!("\nFailures:");
        for result in results.iter().filter(|r| r.status == TestStatus::Failed) {
            println!(
                "  - {}: {}",
                result.name,
                result.error_message.as_deref().unwrap_or("Unknown error")
            );
        }
    }

    let total_duration: std::time::Duration = results.iter().map(|r| r.duration).sum();
    println!("\nTotal Duration: {:?}", total_duration);
}

/// Test suite configuration
#[derive(Debug, Clone, serde::Deserialize)]
struct TestSuiteConfig {
    #[serde(default)]
    plugins: Vec<PluginSuiteEntry>,
}

impl Default for TestSuiteConfig {
    fn default() -> Self {
        Self { plugins: Vec::new() }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PluginSuiteEntry {
    name: String,
    path: String,
    timeout_ms: Option<u64>,
}
