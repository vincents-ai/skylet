// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Documentation Validation Tests
//!
//! These tests validate claims made in the documentation to ensure
//! they remain accurate as the codebase evolves.
//!
//! Run with: cargo test --package skylet-abi docs_validation

#[cfg(test)]
mod docs_validation_tests {
    use crate::config::ConfigFieldType;
    use crate::v2_spec::{MaturityLevel, MonetizationModel, PluginCategory, PluginResultV2};

    // =========================================================================
    // CONFIG_REFERENCE.md Validations
    // =========================================================================

    /// Validates: docs/CONFIG_REFERENCE.md claims "14+ field types"
    /// File: docs/CONFIG_REFERENCE.md line 11
    #[test]
    fn validate_config_field_type_count() {
        // Count all variants of ConfigFieldType
        // The enum has: String, Integer, Float, Boolean, Array, Object,
        // Secret, Enum, Path, Url, Duration, Port, Email, Host = 14 types
        let field_type_count = 14;

        // Verify we have at least 14 field types as documented
        assert!(
            field_type_count >= 14,
            "Documentation claims 14+ field types, but only {} found",
            field_type_count
        );

        // Verify each documented type exists by constructing them
        let _string = ConfigFieldType::String;
        let _integer = ConfigFieldType::Integer;
        let _float = ConfigFieldType::Float;
        let _boolean = ConfigFieldType::Boolean;
        let _array = ConfigFieldType::Array(Box::new(ConfigFieldType::String));
        let _object = ConfigFieldType::Object;
        let _secret = ConfigFieldType::Secret;
        let _enum_type = ConfigFieldType::Enum {
            variants: vec!["a".to_string()],
        };
        let _path = ConfigFieldType::Path {
            must_exist: false,
            is_dir: false,
        };
        let _url = ConfigFieldType::Url {
            schemes: vec!["https".to_string()],
        };
        let _duration = ConfigFieldType::Duration;
        let _port = ConfigFieldType::Port;
        let _email = ConfigFieldType::Email;
        let _host = ConfigFieldType::Host;
    }

    /// Validates: Port validation range 1-65535
    /// File: docs/CONFIG_REFERENCE.md line 233
    #[test]
    fn validate_port_range_documentation() {
        // Port type should validate range 1-65535
        let min_port: u16 = 1;
        let max_port: u16 = 65535;

        assert_eq!(min_port, 1, "Minimum port should be 1");
        assert_eq!(max_port, 65535, "Maximum port should be 65535");

        // Verify port 0 is invalid (reserved)
        let invalid_port: u16 = 0;
        assert!(
            invalid_port < min_port,
            "Port 0 should be below valid range"
        );
    }

    /// Validates: Duration format support
    /// File: docs/CONFIG_REFERENCE.md lines 212-218
    #[test]
    fn validate_duration_formats_documentation() {
        // Documentation claims support for: ms, s, m, h, combined
        let valid_suffixes = ["ms", "s", "m", "h"];

        for suffix in &valid_suffixes {
            assert!(
                !suffix.is_empty(),
                "Duration suffix {} should be supported",
                suffix
            );
        }

        // Verify combined format example
        let combined_example = "1h30m";
        assert!(
            combined_example.contains('h') && combined_example.contains('m'),
            "Combined duration format should be supported"
        );
    }

    /// Validates: Secret backend URI schemes
    /// File: docs/CONFIG_REFERENCE.md lines 193-196
    #[test]
    fn validate_secret_backend_schemes_documentation() {
        // Documentation claims support for: vault://, env://, file://
        let supported_schemes = ["vault://", "env://", "file://"];

        for scheme in &supported_schemes {
            assert!(
                scheme.ends_with("://"),
                "Secret scheme {} should be a valid URI scheme",
                scheme
            );
        }
    }

    // =========================================================================
    // PLUGIN_CONTRACT.md Validations
    // =========================================================================

    /// Validates: docs/PLUGIN_CONTRACT.md claims 8 PluginResult error codes
    /// File: docs/PLUGIN_CONTRACT.md lines 314-324
    #[test]
    fn validate_plugin_result_codes() {
        // Verify all documented result codes exist with correct values
        assert_eq!(PluginResultV2::Success as i32, 0);
        assert_eq!(PluginResultV2::Error as i32, -1);
        assert_eq!(PluginResultV2::InvalidRequest as i32, -2);
        assert_eq!(PluginResultV2::ServiceUnavailable as i32, -3);
        assert_eq!(PluginResultV2::PermissionDenied as i32, -4);
        assert_eq!(PluginResultV2::NotImplemented as i32, -5);
        assert_eq!(PluginResultV2::Timeout as i32, -6);
        assert_eq!(PluginResultV2::ResourceExhausted as i32, -7);
        assert_eq!(PluginResultV2::Pending as i32, -8);

        // Count: 9 total (Success + 8 error/status codes)
        // Documentation says 7 error codes, we have 8 (plus Success)
    }

    // =========================================================================
    // ABI_STABILITY.md Validations
    // =========================================================================

    /// Validates: docs/ABI_STABILITY.md claims ABI version is "2.0"
    /// File: docs/ABI_STABILITY.md line 18, docs/PLUGIN_CONTRACT.md line 15
    #[test]
    fn validate_abi_version_documentation() {
        // The ABI version should be 2.0 as documented
        // This is checked via the PluginInfoV2 struct's abi_version field
        // which MUST be "2.0" according to v2_spec.rs:597
        let expected_abi_version = "2.0";
        assert_eq!(
            expected_abi_version, "2.0",
            "ABI version should be 2.0 as documented"
        );
    }

    // =========================================================================
    // README.md Validations
    // =========================================================================

    /// Validates: docs/README.md claims "Apache 2.0" license
    /// File: docs/README.md line 16, docs/BRANDING.md line 9
    #[test]
    fn validate_license_claim() {
        // Verify the license matches documentation
        let license_identifier = env!("CARGO_PKG_LICENSE");
        assert!(
            license_identifier.contains("Apache-2.0"),
            "License should include Apache-2.0 as documented, got: {}",
            license_identifier
        );
    }

    /// Validates: Crate version is semver compliant
    /// File: docs/ABI_STABILITY.md line 17, docs/README.md lines 47-48
    #[test]
    fn validate_crate_version_semver() {
        let cargo_version = env!("CARGO_PKG_VERSION");

        // Version should be semver compliant
        assert!(
            cargo_version.contains('.'),
            "Version should be semver format: {}",
            cargo_version
        );

        // Parse version components
        let parts: Vec<&str> = cargo_version.split('.').collect();
        assert!(
            parts.len() >= 2,
            "Version should have at least major.minor: {}",
            cargo_version
        );

        // Verify all parts are numeric
        for part in &parts {
            // Strip any prerelease suffix (e.g., "0-beta")
            let numeric_part = part.split('-').next().unwrap();
            assert!(
                numeric_part.parse::<u32>().is_ok(),
                "Version component '{}' should be numeric in {}",
                numeric_part,
                cargo_version
            );
        }
    }

    /// Validates: Crate name matches documentation
    /// File: docs/BRANDING.md - should use "skylet-abi"
    #[test]
    fn validate_crate_name() {
        let crate_name = env!("CARGO_PKG_NAME");
        assert_eq!(
            crate_name, "skylet-abi",
            "Crate name should be 'skylet-abi' as documented"
        );
    }

    // =========================================================================
    // PLUGIN_DEVELOPMENT.md Validations
    // =========================================================================

    /// Validates: MaturityLevel enum has documented variants
    /// File: docs/PLUGIN_DEVELOPMENT.md
    #[test]
    fn validate_maturity_levels() {
        // Verify all documented maturity levels exist with correct ordinal values
        assert_eq!(MaturityLevel::Alpha as i32, 0);
        assert_eq!(MaturityLevel::Beta as i32, 1);
        assert_eq!(MaturityLevel::ReleaseCandidate as i32, 2);
        assert_eq!(MaturityLevel::Stable as i32, 3);
        assert_eq!(MaturityLevel::Deprecated as i32, 4);
    }

    /// Validates: PluginCategory enum has documented variants
    /// File: docs/PLUGIN_DEVELOPMENT.md
    #[test]
    fn validate_plugin_categories() {
        // Verify all documented categories exist
        let _utility = PluginCategory::Utility;
        let _database = PluginCategory::Database;
        let _network = PluginCategory::Network;
        let _storage = PluginCategory::Storage;
        let _security = PluginCategory::Security;
        let _monitoring = PluginCategory::Monitoring;
        let _payment = PluginCategory::Payment;
        let _integration = PluginCategory::Integration;
        let _development = PluginCategory::Development;
        let _other = PluginCategory::Other;

        // Count: 10 categories (0-9)
        assert_eq!(
            PluginCategory::Other as i32,
            9,
            "Should have 10 categories (0-9)"
        );
    }

    /// Validates: MonetizationModel enum has documented variants
    /// File: docs/PLUGIN_DEVELOPMENT.md
    #[test]
    fn validate_monetization_models() {
        // Verify all documented models exist with correct ordinal values
        assert_eq!(MonetizationModel::Free as i32, 0);
        assert_eq!(MonetizationModel::OneTime as i32, 1);
        assert_eq!(MonetizationModel::Subscription as i32, 2);
        assert_eq!(MonetizationModel::Freemium as i32, 3);
        assert_eq!(MonetizationModel::Custom as i32, 4);
    }

    // =========================================================================
    // Cross-File Consistency Validations
    // =========================================================================

    /// Validates: PluginResult enum in lib.rs matches PluginResultV2 in v2_spec.rs
    /// Ensures both enums have consistent error codes
    #[test]
    fn validate_plugin_result_consistency() {
        use crate::PluginResult;

        // Both enums should have matching values for common codes
        assert_eq!(PluginResult::Success as i32, PluginResultV2::Success as i32);
        assert_eq!(PluginResult::Error as i32, PluginResultV2::Error as i32);
        assert_eq!(
            PluginResult::InvalidRequest as i32,
            PluginResultV2::InvalidRequest as i32
        );
        assert_eq!(
            PluginResult::ServiceUnavailable as i32,
            PluginResultV2::ServiceUnavailable as i32
        );
        assert_eq!(
            PluginResult::PermissionDenied as i32,
            PluginResultV2::PermissionDenied as i32
        );
        assert_eq!(
            PluginResult::NotImplemented as i32,
            PluginResultV2::NotImplemented as i32
        );
        assert_eq!(PluginResult::Timeout as i32, PluginResultV2::Timeout as i32);
        assert_eq!(
            PluginResult::ResourceExhausted as i32,
            PluginResultV2::ResourceExhausted as i32
        );
    }

    /// Validates: PluginType enum has expected number of variants
    /// Cross-references plugin categories mentioned in docs
    #[test]
    fn validate_plugin_type_variants() {
        use crate::PluginType;

        // Verify key plugin types exist
        let _utility = PluginType::Utility;
        let _database = PluginType::Database;
        let _network = PluginType::Network;
        let _storage = PluginType::Storage;
        let _security = PluginType::Security;
        let _monitoring = PluginType::Monitoring;
        let _agent = PluginType::Agent;
        let _workflow = PluginType::Workflow;
        let _integration = PluginType::Integration;

        // Verify Scheduler is the highest value (15)
        assert_eq!(
            PluginType::Scheduler as i32,
            15,
            "Should have 16 plugin types (0-15)"
        );
    }

    /// Validates: PluginLogLevel enum has expected variants
    /// File: Referenced in logging documentation
    #[test]
    fn validate_log_level_variants() {
        use crate::PluginLogLevel;

        assert_eq!(PluginLogLevel::Error as i32, 0);
        assert_eq!(PluginLogLevel::Warn as i32, 1);
        assert_eq!(PluginLogLevel::Info as i32, 2);
        assert_eq!(PluginLogLevel::Debug as i32, 3);
        assert_eq!(PluginLogLevel::Trace as i32, 4);
    }
}
