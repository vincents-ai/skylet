// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Plugin Tester - Configurable plugin testing tool
//!
//! Tests multiple plugins from a config file with detailed reporting.
//!
//! # Usage
//!
//! ```bash
//! # Test all plugins from config
//! plugin-tester test --config plugins.toml
//!
//! # Test single plugin
//! plugin-tester test-single --path ./target/release/libplatform_detect.so
//!
//! # List available tests in config
//! plugin-tester list --config plugins.toml
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use libloading::{Library, Symbol};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "plugin-tester")]
#[command(about = "Configurable plugin testing tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Test all plugins from config file
    Test {
        /// Path to plugins config file
        #[arg(short, long, default_value = "plugins.toml")]
        config: String,
    },
    /// Test a single plugin
    TestSingle {
        /// Path to plugin shared library
        #[arg(short, long)]
        path: String,
        /// Optional test name to run
        #[arg(short, long)]
        test: Option<String>,
    },
    /// List available tests in config
    List {
        /// Path to plugins config file
        #[arg(short, long, default_value = "plugins.toml")]
        config: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct PluginConfig {
    name: String,
    path: String,
    tests: Vec<TestCase>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TestCase {
    name: String,
    #[serde(default)]
    expected_status: String,
    #[serde(default)]
    timeout_ms: u64,
    #[serde(default)]
    expected_output: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct PluginsConfig {
    plugins: Vec<PluginConfig>,
}

fn load_plugin(path: &str) -> Result<Library> {
    info!("Loading plugin: {}", path);
    unsafe { Library::new(path).map_err(|e| anyhow::anyhow!("Failed to load {}: {}", path, e)) }
}

fn test_plugin(path: &str, tests: &[TestCase]) -> Result<TestResults> {
    let library = load_plugin(path)?;

    let mut results = TestResults {
        plugin: path.to_string(),
        passed: 0,
        failed: 0,
        tests: Vec::new(),
    };

    for test in tests {
        info!("Running test: {}", test.name);

        let test_result = run_test(&library, test);
        results.tests.push(test_result.clone());

        if test_result.passed {
            results.passed += 1;
            info!("  ✓ {}", test.name);
        } else {
            results.failed += 1;
            error!(
                "  ✗ {}: {}",
                test.name,
                test_result.error.as_deref().unwrap_or("unknown")
            );
        }
    }

    Ok(results)
}

#[derive(Debug, Clone)]
struct TestResult {
    name: String,
    passed: bool,
    duration_ms: u64,
    error: Option<String>,
}

fn run_test(library: &Library, test: &TestCase) -> TestResult {
    let start = std::time::Instant::now();

    type InitFn = unsafe extern "C" fn(*const ()) -> i32;
    type ShutdownFn = unsafe extern "C" fn(*const ()) -> i32;

    let init: Result<Symbol<InitFn>, _> = unsafe { library.get(b"plugin_init_v2") };

    match init {
        Ok(init_fn) => {
            let result = unsafe { init_fn(std::ptr::null()) };

            let duration = start.elapsed().as_millis() as u64;

            let passed = match test.expected_status.as_str() {
                "success" | "" => result == 0,
                _ => false,
            };

            if passed {
                // Try shutdown
                if let Ok(shutdown_fn) =
                    unsafe { library.get::<Symbol<ShutdownFn>>(b"plugin_shutdown_v2") }
                {
                    unsafe { shutdown_fn(std::ptr::null()) };
                }
            }

            TestResult {
                name: test.name.clone(),
                passed,
                duration_ms: duration,
                error: if passed {
                    None
                } else {
                    Some(format!("init returned {}", result))
                },
            }
        }
        Err(e) => TestResult {
            name: test.name.clone(),
            passed: false,
            duration_ms: start.elapsed().as_millis() as u64,
            error: Some(format!("Failed to load plugin_init_v2: {}", e)),
        },
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct TestResults {
    plugin: String,
    passed: usize,
    failed: usize,
    tests: Vec<TestResult>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Test { config } => {
            info!("Loading config: {}", config);
            let content = std::fs::read_to_string(&config)?;
            let plugins: PluginsConfig = toml::from_str(&content)?;

            let mut total_passed = 0;
            let mut total_failed = 0;

            for plugin in &plugins.plugins {
                println!("\n📦 Testing: {} ({})", plugin.name, plugin.path);
                println!("{}", "-".repeat(50));

                match test_plugin(&plugin.path, &plugin.tests) {
                    Ok(results) => {
                        total_passed += results.passed;
                        total_failed += results.failed;

                        for test in &results.tests {
                            if test.passed {
                                println!("  ✓ {} ({}ms)", test.name, test.duration_ms);
                            } else {
                                println!(
                                    "  ✗ {}: {}",
                                    test.name,
                                    test.error.as_deref().unwrap_or("failed")
                                );
                            }
                        }

                        println!(
                            "  Results: {} passed, {} failed",
                            results.passed, results.failed
                        );
                    }
                    Err(e) => {
                        error!("Failed to test {}: {}", plugin.name, e);
                        total_failed += 1;
                    }
                }
            }

            println!("\n{}", "=".repeat(50));
            println!("Total: {} passed, {} failed", total_passed, total_failed);

            if total_failed > 0 {
                std::process::exit(1);
            }
        }
        Commands::TestSingle { path, test } => {
            let tests = if let Some(name) = test {
                vec![TestCase {
                    name,
                    expected_status: "success".to_string(),
                    timeout_ms: 5000,
                    expected_output: None,
                }]
            } else {
                vec![TestCase {
                    name: "init".to_string(),
                    expected_status: "success".to_string(),
                    timeout_ms: 5000,
                    expected_output: None,
                }]
            };

            match test_plugin(&path, &tests) {
                Ok(results) => {
                    for test in &results.tests {
                        if test.passed {
                            println!("✓ {} ({}ms)", test.name, test.duration_ms);
                        } else {
                            println!(
                                "✗ {}: {}",
                                test.name,
                                test.error.as_deref().unwrap_or("failed")
                            );
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    error!("Test failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::List { config } => {
            let content = std::fs::read_to_string(&config)?;
            let plugins: PluginsConfig = toml::from_str(&content)?;

            println!("Available plugins and tests:\n");
            for plugin in &plugins.plugins {
                println!("📦 {}", plugin.name);
                println!("   Path: {}", plugin.path);
                for test in &plugin.tests {
                    println!("   - {}", test.name);
                }
                println!();
            }
        }
    }

    Ok(())
}
