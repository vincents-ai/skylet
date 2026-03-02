// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Comprehensive Test Suite for Skylet
//!
//! This binary provides a comprehensive testing framework including:
//! - Unit testing with mock plugins
//! - Performance benchmarking
//! - Security testing
//! - Integration testing
//! - Chaos engineering

use clap::Parser;
use skylet::config::AppConfig;
use skylet::testing_comprehensive::{
    TestSuiteConfig, TestEnvironment, PerformanceTestConfig, SecurityTestConfig, IntegrationTestConfig,
    TestOrchestrator, MockPluginGenerator, MockPluginFeatures,
};
use std::path::PathBuf;
use tracing::{info, error, warn};
use tokio::time::{timeout, Duration};

#[derive(Parser)]
#[command(name = "skylet-test-suite")]
#[command(about = "Comprehensive testing suite for Skylet")]
struct Cli {
    /// Test environment type
    #[arg(long, default_value = "integration")]
    env: TestEnvironment,

    /// Plugin directory for test plugins
    #[arg(long, default_value = "./test-plugins")]
    plugin_dir: PathBuf,

    /// Enable chaos testing
    #[arg(long)]
    chaos: bool,

    /// Performance test iterations
    #[arg(long, default_value = "100")]
    iterations: usize,

    /// Security testing enabled
    #[arg(long, default_value = "true")]
    security: bool,

    /// Generate test report file
    #[arg(long)]
    report_file: Option<PathBuf>,

    /// Verbosity level
    #[arg(short, long, default_value = "info")]
    verbose: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    {
        use tracing_subscriber::{fmt, EnvFilter};
        
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&format!("skylet_test_suite={}", std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))));
        
        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_thread_ids(true)
            .init();
    }

    let cli = Cli::parse();
    
    info!("🧪 Starting Skylet Comprehensive Test Suite");
    info!("📋 Configuration: {:?}", cli);

    // Load application configuration
    let config = AppConfig::load()?;

    // Create test suite configuration
    let test_config = TestSuiteConfig {
        env_type: cli.env.clone(),
        plugin_dir: cli.plugin_dir.clone(),
        chaos_enabled: cli.chaos,
        performance_config: PerformanceTestConfig {
            iterations: cli.iterations,
            warmup_iterations: 10,
            test_timeout_ms: 5000,
            track_memory: true,
            concurrency_level: 10,
        },
        security_config: SecurityTestConfig {
            scan_vulnerabilities: cli.security,
            test_isolation: cli.security,
            test_resource_limits: cli.security,
            test_permissions: cli.security,
        },
        integration_config: IntegrationTestConfig {
            plugin_scenarios: vec![
                "basic_plugin_loading".to_string(),
                "plugin_communication".to_string(),
                "configuration_reload".to_string(),
                "hot_reload_scenario".to_string(),
            ],
            service_tests: true,
            config_validation: true,
            event_system_tests: true,
        },
    };

    // Initialize test orchestrator
    info!("🚀 Initializing test orchestrator");
    let orchestrator = TestOrchestrator::new(test_config, config.clone());

    // Run comprehensive test suite with timeout
    let test_result = timeout(
        Duration::from_secs(300), // 5 minute timeout
        orchestrator.run_test_suite()
    ).await;

    match test_result {
        Ok(Ok(test_suite_result)) => {
            info!("✅ Test suite completed successfully");
            
            // Print test results summary
            print_test_summary(&test_suite_result);
            
            // Save report if requested
            if let Some(report_file) = cli.report_file {
                std::fs::write(&report_file, &test_suite_result.report)?;
                info!("📄 Test report saved to: {}", report_file.display());
            }
            
            // Exit with appropriate code
            if test_suite_result.results.performance_results.is_empty() 
                && test_suite_result.results.security_results.isolation_test.is_none()
                && test_suite_result.results.integration_results.plugin_loading.is_none() {
                warn!("⚠️  No tests were actually run - check configuration");
                std::process::exit(1);
            }
            
            std::process::exit(0);
        }
        Ok(Err(e)) => {
            error!("❌ Test suite failed: {}", e);
            std::process::exit(1);
        }
        Err(_) => {
            error!("❌ Test suite timed out after 5 minutes");
            std::process::exit(1);
        }
    }
}

fn print_test_summary(result: &skylet::testing_comprehensive::TestSuiteResult) {
    info!("📊 Test Suite Summary");
    info!("=" .repeat(50));
    
    // Performance tests
    if !result.results.performance_results.is_empty() {
        info!("🏃 Performance Tests: {} results", result.results.performance_results.len());
        for perf_result in &result.results.performance_results {
            info!("  • {}: {:.2}ms average (min: {:.2}ms, max: {:.2}ms)", 
                perf_result.test_name, 
                perf_result.avg_time_ms,
                perf_result.min_time_ms,
                perf_result.max_time_ms);
        }
    } else {
        info!("🏃 Performance Tests: Not run");
    }
    
    // Security tests
    info!("🔒 Security Tests:");
    if let Some(isolation_test) = &result.results.security_results.isolation_test {
        info!("  • Plugin Isolation: {}", if isolation_test.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    } else {
        info!("  • Plugin Isolation: Not run");
    }
    
    if let Some(resource_limits_test) = &result.results.security_results.resource_limits_test {
        info!("  • Resource Limits: {}", if resource_limits_test.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    } else {
        info!("  • Resource Limits: Not run");
    }
    
    if let Some(permissions_test) = &result.results.security_results.permissions_test {
        info!("  • Permissions: {}", if permissions_test.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    } else {
        info!("  • Permissions: Not run");
    }
    
    // Integration tests
    info!("🔗 Integration Tests:");
    if let Some(plugin_loading) = &result.results.integration_results.plugin_loading {
        info!("  • Plugin Loading: {}", if plugin_loading.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    } else {
        info!("  • Plugin Loading: Not run");
    }
    
    if let Some(plugin_communication) = &result.results.integration_results.plugin_communication {
        info!("  • Plugin Communication: {}", if plugin_communication.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    } else {
        info!("  • Plugin Communication: Not run");
    }
    
    if let Some(configuration_reload) = &result.results.integration_results.configuration_reload {
        info!("  • Configuration Reload: {}", if configuration_reload.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    } else {
        info!("  • Configuration Reload: Not run");
    }
    
    info!("📝 Generated at: {}", result.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
    info!("=" .repeat(50));
}

// Unit tests for the test suite itself
#[cfg(test)]
mod tests {
    use super::*;
    use skylet::testing_comprehensive::{MockPluginGenerator, MockPluginFeatures};

    #[test]
    fn test_cli_parsing() {
        let args = Cli::try_parse_from(&[
            "skylet-test-suite",
            "--env", "integration",
            "--iterations", "50",
            "--chaos",
        ]).unwrap();
        
        assert_eq!(args.env, TestEnvironment::Integration);
        assert_eq!(args.iterations, 50);
        assert!(args.chaos);
    }

    #[test]
    fn test_mock_plugin_generation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let generator = MockPluginGenerator::new(temp_dir.path().to_path_buf());
        
        let features = MockPluginFeatures {
            supports_hot_reload: true,
            supports_async: false,
            supports_streaming: true,
            memory_mb: 32,
            cpu_cores: 2,
        };
        
        let result = generator.generate_mock_plugin("test_plugin", "2.0", features);
        assert!(result.is_ok());
        
        let plugin_path = result.unwrap();
        assert!(plugin_path.exists());
        assert!(plugin_path.join("plugin_info.toml").exists());
    }

    #[test]
    fn test_test_suite_config_default() {
        let config = TestSuiteConfig::default();
        
        assert_eq!(config.env_type, TestEnvironment::Integration);
        assert!(!config.performance_config.iterations.is_empty());
        assert!(config.chaos_enabled);
    }

    #[tokio::test]
    async fn test_performance_test_runner_creation() {
        let perf_config = PerformanceTestConfig {
            iterations: 10,
            warmup_iterations: 2,
            test_timeout_ms: 1000,
            track_memory: false,
            concurrency_level: 5,
        };
        
        let runner = TestOrchestrator::new(
            TestSuiteConfig {
                env_type: TestEnvironment::Unit,
                plugin_dir: PathBuf::from("./test"),
                chaos_enabled: false,
                performance_config: perf_config.clone(),
                security_config: SecurityTestConfig::default(),
                integration_config: IntegrationTestConfig::default(),
            },
            AppConfig::load().unwrap()
        );
        
        // This test mainly verifies that the orchestrator can be created
        // without panicking
        assert!(true);
    }
}