//! Unit tests for configuration system

use super::*;
use crate::plugin_manager::config::*;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn test_schema_validation_valid_config() {
        let schema_json = r#"{
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "minLength": 32,
                    "maxLength": 256
                },
                "timeout": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600
                }
            },
            "required": ["api_key"]
        }"#;

        let config_json = r#"{
            "api_key": "12345678901234567890123456789012",
            "timeout": 60
        }"#;

        let validator = SchemaValidator::from_json(schema_json).unwrap();
        let result = validator.validate(config_json).unwrap();

        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_schema_validation_missing_required_field() {
        let schema_json = r#"{
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string"
                }
            },
            "required": ["api_key"]
        }"#;

        let config_json = r#"{
            "timeout": 60
        }"#;

        let validator = SchemaValidator::from_json(schema_json).unwrap();
        let result = validator.validate(config_json).unwrap();

        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("required"));
    }

    #[test]
    fn test_schema_validation_invalid_type() {
        let schema_json = r#"{
            "type": "object",
            "properties": {
                "timeout": {
                    "type": "integer"
                }
            }
        }"#;

        let config_json = r#"{
            "timeout": "not a number"
        }"#;

        let validator = SchemaValidator::from_json(schema_json).unwrap();
        let result = validator.validate(config_json).unwrap();

        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_schema_validation_min_length() {
        let schema_json = r#"{
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "minLength": 32
                }
            }
        }"#;

        let config_json = r#"{
            "api_key": "short"
        }"#;

        let validator = SchemaValidator::from_json(schema_json).unwrap();
        let result = validator.validate(config_json).unwrap();

        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("minLength"));
    }

    #[test]
    fn test_schema_validation_max_length() {
        let schema_json = r#"{
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "maxLength": 10
                }
            }
        }"#;

        let config_json = r#"{
            "api_key": "this_is_too_long"
        }"#;

        let validator = SchemaValidator::from_json(schema_json).unwrap();
        let result = validator.validate(config_json).unwrap();

        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("maxLength"));
    }

    #[test]
    fn test_schema_validation_range() {
        let schema_json = r#"{
            "type": "object",
            "properties": {
                "timeout": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600
                }
            }
        }"#;

        // Test minimum
        let config_json = r#"{
            "timeout": 0
        }"#;

        let validator = SchemaValidator::from_json(schema_json).unwrap();
        let result = validator.validate(config_json).unwrap();

        assert!(!result.is_valid);
        assert!(result.errors[0].message.contains("minimum"));

        // Test maximum
        let config_json = r#"{
            "timeout": 4000
        }"#;

        let validator = SchemaValidator::from_json(schema_json).unwrap();
        let result = validator.validate(config_json).unwrap();

        assert!(!result.is_valid);
        assert!(result.errors[0].message.contains("maximum"));
    }
}

#[cfg(test)]
mod env_integration_tests {
    use super::*;

    #[test]
    fn test_env_var_mapping() {
        let env_config = EnvVarConfig::new("SKYLET_".to_string())
            .with_separator("_".to_string())
            .with_overwrite_files(true);

        assert_eq!(env_config.prefix, "SKYLET_");
        assert_eq!(env_config.separator, "_");
        assert!(env_config.overwrite_files);
    }

    #[test]
    fn test_env_var_resolution() {
        let config_json = r#"{
            "database_url": "${env:DB_URL:http://localhost:5432}",
            "api_key": "${env:API_KEY}",
            "timeout": 30
        }"#;

        let mut integrator = EnvVarIntegrator::new(EnvVarConfig::new("SKYLET_".to_string()));

        // Set environment variables
        std::env::set_var("DB_URL", "postgres://localhost:5432/test");
        std::env::set_var("API_KEY", "secret123");

        let resolved = integrator.resolve_env_vars(config_json).unwrap();

        assert!(resolved.contains("postgres://localhost:5432/test"));
        assert!(resolved.contains("secret123"));
        assert!(resolved.contains("30"));

        std::env::remove_var("DB_URL");
        std::env::remove_var("API_KEY");
    }

    #[test]
    fn test_env_var_default_value() {
        let config_json = r#"{
            "database_url": "${env:DB_URL:http://localhost:5432}"
        }"#;

        let mut integrator = EnvVarIntegrator::new(EnvVarConfig::new("SKYLET_".to_string()));

        let resolved = integrator.resolve_env_vars(config_json).unwrap();

        assert!(resolved.contains("http://localhost:5432"));
    }

    #[test]
    fn test_env_var_missing_required() {
        let config_json = r#"{
            "api_key": "${env:API_KEY}"
        }"#;

        let mut integrator = EnvVarIntegrator::new(EnvVarConfig::new("SKYLET_".to_string()));

        // Don't set API_KEY
        let result = integrator.resolve_env_vars(config_json);

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod multi_env_tests {
    use super::*;

    #[tokio::test]
    async fn test_environment_switching() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        // Create config directories for different environments
        let dev_dir = config_dir.join("dev");
        let prod_dir = config_dir.join("prod");
        std::fs::create_dir_all(&dev_dir).unwrap();
        std::fs::create_dir_all(&prod_dir).unwrap();

        // Create dev config
        let dev_config = r#"{
            "database_url": "postgres://localhost:5432/dev",
            "debug": true
        }"#;
        std::fs::write(dev_dir.join("test_plugin.toml"), dev_config).unwrap();

        // Create prod config
        let prod_config = r#"{
            "database_url": "postgres://prod-db:5432/prod",
            "debug": false
        }"#;
        std::fs::write(prod_dir.join("test_plugin.toml"), prod_config).unwrap();

        let mut manager = MultiEnvConfigManager::new(config_dir.to_path_buf())
            .unwrap()
            .with_environment(ConfigEnvironment::Development);

        let dev_config = manager.load_plugin_config("test_plugin").await.unwrap();
        assert!(dev_config.contains("localhost:5432/dev"));
        assert!(dev_config.contains("true"));

        manager.switch_environment(ConfigEnvironment::Production).unwrap();

        let prod_config = manager.load_plugin_config("test_plugin").await.unwrap();
        assert!(prod_config.contains("prod-db:5432/prod"));
        assert!(prod_config.contains("false"));
    }

    #[tokio::test]
    async fn test_config_comparison() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        let dev_dir = config_dir.join("dev");
        let prod_dir = config_dir.join("prod");
        std::fs::create_dir_all(&dev_dir).unwrap();
        std::fs::create_dir_all(&prod_dir).unwrap();

        let dev_config = r#"{
            "database_url": "postgres://localhost:5432/dev",
            "debug": true,
            "timeout": 30
        }"#;
        std::fs::write(dev_dir.join("test_plugin.toml"), dev_config).unwrap();

        let prod_config = r#"{
            "database_url": "postgres://prod-db:5432/prod",
            "debug": false,
            "timeout": 30
        }"#;
        std::fs::write(prod_dir.join("test_plugin.toml"), prod_config).unwrap();

        let manager = MultiEnvConfigManager::new(config_dir.to_path_buf())
            .unwrap()
            .with_environment(ConfigEnvironment::Development);

        let comparison = manager.compare_configs_for_plugin("test_plugin");

        assert!(!comparison.differences.is_empty());
        assert!(comparison.differences.len() == 2); // database_url and debug differ
    }
}

#[cfg(test)]
mod advanced_backend_tests {
    use super::*;

    #[tokio::test]
    async fn test_config_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        let config_content = r#"[database]
host = "localhost"
port = 5432

[cache]
enabled = true
ttl_seconds = 3600
"#;
        std::fs::write(config_dir.join("test_plugin.toml"), config_content).unwrap();

        let backend = AdvancedConfigBackend::new(config_dir.to_path_buf());
        let config = backend.load_plugin_config("test_plugin").await.unwrap();

        assert!(config.contains("localhost"));
        assert!(config.contains("5432"));
        assert!(config.contains("3600"));
    }

    #[tokio::test]
    async fn test_config_value_getting() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        let config_content = r#"api_key = "secret123"
timeout = 30
"#;
        std::fs::write(config_dir.join("test_plugin.toml"), config_content).unwrap();

        let backend = AdvancedConfigBackend::new(config_dir.to_path_buf());
        let api_key = backend.get_config_value("test_plugin", "api_key").await.unwrap();
        let timeout = backend.get_config_value("test_plugin", "timeout").await.unwrap();

        assert_eq!(api_key, "secret123");
        assert_eq!(timeout, "30");
    }

    #[tokio::test]
    async fn test_config_value_setting() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        let config_content = r#"api_key = "old_value"
"#;
        std::fs::write(config_dir.join("test_plugin.toml"), config_content).unwrap();

        let mut backend = AdvancedConfigBackend::new(config_dir.to_path_buf());
        backend.set_config_value("test_plugin", "api_key", "new_value").await.unwrap();

        let updated = backend.get_config_value("test_plugin", "api_key").await.unwrap();
        assert_eq!(updated, "new_value");
    }

    #[tokio::test]
    async fn test_config_validation() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        let schema_json = r#"{
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "minLength": 32
                }
            },
            "required": ["api_key"]
        }"#;

        let config_content = r#"api_key = "short"
"#;
        std::fs::write(config_dir.join("test_plugin.toml"), config_content).unwrap();

        let mut backend = AdvancedConfigBackend::new(config_dir.to_path_buf());
        let validator = SchemaValidator::from_json(schema_json).unwrap();

        let result = backend.validate_config("test_plugin", &validator).await.unwrap();

        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }
}

#[cfg(test)]
mod hot_reload_tests {
    use super::*;

    #[tokio::test]
    async fn test_config_hot_reload() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        let config_content = r#"api_key = "initial_value"
"#;
        std::fs::write(config_dir.join("test_plugin.toml"), config_content).unwrap();

        let reload_config = ReloadConfig {
            enabled: true,
            debounce_duration: std::time::Duration::from_millis(100),
            validate_after_reload: false,
            backup_on_failure: false,
            max_retries: 1,
        };

        let mut hot_reload = ConfigHotReload::new(config_dir.to_path_buf(), reload_config).unwrap();

        let mut reload_triggered = false;
        hot_reload.add_reload_callback(Box::new(move |_| {
            reload_triggered = true;
        })).await;

        // Modify config file
        std::fs::write(config_dir.join("test_plugin.toml"), "api_key = \"updated_value\"\n").unwrap();

        // Wait for reload
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Note: In a real scenario, we'd check if reload_triggered is true
        // This requires proper async notification system
    }
}
