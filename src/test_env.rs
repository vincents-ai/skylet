// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use crate::config::{AppConfig, ConfigArgs};
use config::{Config, Environment};
use std::env;
use tracing;

#[cfg(test)]
mod test_env_vars {
    use super::*;

    #[test]
    fn test_simple_environment_variable_loading() {
        // Try with a simple flat structure first
        env::set_var("AUTONOMOUS_TEST", "test_value");

        let settings = Config::builder()
            .add_source(Environment::with_prefix("AUTONOMOUS"))
            .build()
            .unwrap();

        match settings.get_string("test") {
            Ok(value) => tracing::info!("Simple env var test: {}", value),
            Err(e) => tracing::info!("Error getting simple env var: {}", e),
        }

        env::remove_var("AUTONOMOUS_TEST");
    }

    #[test]
    fn test_nested_environment_variable_loading() {
        env::set_var("AUTONOMOUS_DATABASE__PATH", "/tmp/test_env.db");

        // Try without explicit separator first
        let settings = Config::builder()
            .set_default("database.path", "./data/marketplace.db")
            .unwrap()
            .add_source(Environment::with_prefix("AUTONOMOUS"))
            .build()
            .unwrap();

        match settings.get_string("database.path") {
            Ok(value) => tracing::info!("Nested env var test (no explicit sep): {}", value),
            Err(e) => tracing::info!("Error getting nested env var (no explicit sep): {}", e),
        }

        env::remove_var("AUTONOMOUS_DATABASE__PATH");
    }

    #[test]
    fn test_nested_environment_variable_loading_with_sep() {
        env::set_var("AUTONOMOUS_DATABASE__PATH", "/tmp/test_env.db");

        // Try with explicit separator
        let settings = Config::builder()
            .set_default("database.path", "./data/marketplace.db")
            .unwrap()
            .add_source(Environment::with_prefix("AUTONOMOUS").separator("__"))
            .build()
            .unwrap();

        match settings.get_string("database.path") {
            Ok(value) => tracing::info!("Nested env var test (with sep): {}", value),
            Err(e) => tracing::info!("Error getting nested env var (with sep): {}", e),
        }

        env::remove_var("AUTONOMOUS_DATABASE__PATH");
    }

    #[test]
    fn test_nested_environment_variable_loading_single_underscore() {
        env::set_var("AUTONOMOUS_DATABASE_PATH", "/tmp/test_env_single.db");

        // Try with single underscore
        let settings = Config::builder()
            .set_default("database.path", "./data/marketplace.db")
            .unwrap()
            .add_source(Environment::with_prefix("AUTONOMOUS").separator("_"))
            .build()
            .unwrap();

        match settings.get_string("database.path") {
            Ok(value) => tracing::info!("Nested env var test (single underscore): {}", value),
            Err(e) => tracing::info!("Error getting nested env var (single underscore): {}", e),
        }

        env::remove_var("AUTONOMOUS_DATABASE_PATH");
    }

    #[test]
    fn test_environment_variable_formats() {
        // Test different environment variable formats
        let test_cases = vec![
            ("AUTONOMOUS_DATABASE_PATH", "/tmp/test1.db"),
            ("AUTONOMOUS_DATABASE__PATH", "/tmp/test2.db"),
            ("AUTONOMOUS_DATABASE_PATH", "/tmp/test3.db"),
        ];

        for (i, (env_var, expected_value)) in test_cases.iter().enumerate() {
            env::set_var(env_var, expected_value);

            // Test with default separator
            let settings = Config::builder()
                .set_default("database.path", "./data/marketplace.db")
                .unwrap()
                .add_source(Environment::with_prefix("AUTONOMOUS"))
                .build()
                .unwrap();

            match settings.get_string("database.path") {
                Ok(value) => tracing::info!("Test case {}: {} = {}", i + 1, env_var, value),
                Err(e) => tracing::info!("Test case {}: {} - Error: {}", i, env_var, e),
            }

            env::remove_var(env_var);
        }
    }

    #[test]
    fn test_environment_variable_loading() {
        env::set_var("AUTONOMOUS_DATABASE_PATH", "/tmp/test_env.db");
        env::set_var("AUTONOMOUS_TOR_SOCKS_PORT", "9999");

        let args = ConfigArgs {
            config: None,
            database_path: None,
            database_node_id: None,
            database_raft_nodes: None,
            database_election_timeout_ms: None,
            database_secret_raft: None,
            database_secret_api: None,
            database_data_dir: None,
            tor_socks_port: None,
            tor_control_port: None,
            tor_hidden_service_port: None,
            monero_daemon_url: None,
            monero_wallet_path: None,
            monero_wallet_rpc_port: None,
            monero_network: None,
            monero_wallet_password: None,
            monero_auto_refresh: None,
            monero_refresh_interval: None,
            agents_enabled: None,
            agents_security_scan_interval: None,
            agents_maintenance_interval: None,
            discovery_enabled: None,
            discovery_dht_bootstrap_nodes: None,
            discovery_dht_replication_factor: None,
            discovery_dht_timeout_seconds: None,
            discovery_i2p_enabled: None,
            discovery_i2p_sam_host: None,
            discovery_i2p_sam_port: None,
            discovery_fallback_endpoints: None,
            discovery_cache_ttl: None,
            discovery_announce_interval: None,
            discovery_peer_id: None,
            discovery_private_key: None,
            payments_enabled: None,
            payments_reserve_months: None,
            payments_payment_buffer_days: None,
            payments_check_interval_hours: None,
            payments_max_payment_amount: None,
            payments_retry_attempts: None,
            payments_retry_delay_minutes: None,
            payments_large_payment_threshold: None,
            escrow_default_release_days: None,
            escrow_max_dispute_days: None,
            escrow_marketplace_fee_percentage: None,
            escrow_min_arbiter_confidence: None,
            escrow_auto_arbitration_enabled: None,
        };

        let config = AppConfig::load(&args).unwrap();
        assert_eq!(
            config.database.path,
            std::path::PathBuf::from("/tmp/test_env.db")
        );
        assert_eq!(config.tor.socks_port, 9999);

        env::remove_var("AUTONOMOUS_DATABASE_PATH");
        env::remove_var("AUTONOMOUS_TOR_SOCKS_PORT");
    }
}
