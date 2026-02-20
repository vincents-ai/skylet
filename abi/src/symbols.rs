// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Symbol Resolution and Function Signature Validation
//!
//! This module provides comprehensive symbol resolution capabilities for loaded plugins,
//! including function signature validation, export enumeration, and symbol caching.
//!
//! RFC-0004 Phase 2: Dynamic Plugin Loading - Week 2

use std::collections::HashMap;
use std::fmt;

/// Represents a function signature for validation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionSignature {
    /// Name of the function
    pub name: String,

    /// Return type identifier
    pub return_type: String,

    /// Parameter types
    pub params: Vec<String>,

    /// Whether function is optional
    pub is_optional: bool,
}

impl FunctionSignature {
    /// Create a new function signature
    pub fn new(name: impl Into<String>, return_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            return_type: return_type.into(),
            params: Vec::new(),
            is_optional: false,
        }
    }

    /// Add a parameter to the signature
    pub fn with_param(mut self, param_type: impl Into<String>) -> Self {
        self.params.push(param_type.into());
        self
    }

    /// Mark this signature as optional
    pub fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }

    /// Get a human-readable signature string
    pub fn signature_string(&self) -> String {
        let params = self.params.join(", ");
        format!("{}({}) -> {}", self.name, params, self.return_type)
    }

    /// Validate another signature against this one
    ///
    /// Returns Ok if signatures match, Err with details if they don't
    pub fn validate(&self, other: &FunctionSignature) -> Result<(), SignatureError> {
        // Check name matches
        if self.name != other.name {
            return Err(SignatureError::NameMismatch {
                expected: self.name.clone(),
                found: other.name.clone(),
            });
        }

        // Check return type matches
        if self.return_type != other.return_type {
            return Err(SignatureError::ReturnTypeMismatch {
                expected: self.return_type.clone(),
                found: other.return_type.clone(),
            });
        }

        // Check parameter count matches
        if self.params.len() != other.params.len() {
            return Err(SignatureError::ParameterCountMismatch {
                expected: self.params.len(),
                found: other.params.len(),
            });
        }

        // Check each parameter type matches
        for (i, (expected, found)) in self.params.iter().zip(other.params.iter()).enumerate() {
            if expected != found {
                return Err(SignatureError::ParameterTypeMismatch {
                    param_index: i,
                    expected: expected.clone(),
                    found: found.clone(),
                });
            }
        }

        Ok(())
    }
}

impl fmt::Display for FunctionSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.signature_string())
    }
}

/// Errors that can occur during signature validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureError {
    /// Function name doesn't match
    NameMismatch { expected: String, found: String },

    /// Return type doesn't match
    ReturnTypeMismatch { expected: String, found: String },

    /// Parameter count doesn't match
    ParameterCountMismatch { expected: usize, found: usize },

    /// A parameter type doesn't match
    ParameterTypeMismatch {
        param_index: usize,
        expected: String,
        found: String,
    },
}

impl fmt::Display for SignatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignatureError::NameMismatch { expected, found } => {
                write!(
                    f,
                    "Function name mismatch: expected '{}', found '{}'",
                    expected, found
                )
            }
            SignatureError::ReturnTypeMismatch { expected, found } => {
                write!(
                    f,
                    "Return type mismatch: expected '{}', found '{}'",
                    expected, found
                )
            }
            SignatureError::ParameterCountMismatch { expected, found } => {
                write!(
                    f,
                    "Parameter count mismatch: expected {}, found {}",
                    expected, found
                )
            }
            SignatureError::ParameterTypeMismatch {
                param_index,
                expected,
                found,
            } => {
                write!(
                    f,
                    "Parameter {} type mismatch: expected '{}', found '{}'",
                    param_index, expected, found
                )
            }
        }
    }
}

impl std::error::Error for SignatureError {}

/// A resolved symbol with its function pointer and metadata
#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    /// Name of the symbol
    pub name: String,

    /// Validated signature
    pub signature: FunctionSignature,

    /// Address of the symbol (as raw pointer)
    pub address: usize,

    /// Whether this symbol is required or optional
    pub is_required: bool,

    /// Whether this symbol was actually resolved
    pub is_resolved: bool,
}

impl ResolvedSymbol {
    /// Create a new resolved symbol
    pub fn new(name: impl Into<String>, signature: FunctionSignature, address: usize) -> Self {
        Self {
            name: name.into(),
            signature,
            address,
            is_required: true,
            is_resolved: true,
        }
    }

    /// Mark this symbol as optional
    pub fn optional(mut self) -> Self {
        self.is_required = false;
        self
    }

    /// Create an unresolved symbol placeholder
    pub fn unresolved(name: impl Into<String>, signature: FunctionSignature) -> Self {
        Self {
            name: name.into(),
            signature,
            address: 0,
            is_required: true,
            is_resolved: false,
        }
    }
}

/// Symbol registry for tracking and validating resolved symbols
#[derive(Debug, Clone)]
pub struct SymbolRegistry {
    /// Map of symbol name to resolved symbol
    symbols: HashMap<String, ResolvedSymbol>,

    /// Expected signatures for validation
    expected_signatures: HashMap<String, FunctionSignature>,

    /// Total number of required symbols
    required_count: usize,

    /// Number of actually resolved symbols
    resolved_count: usize,
}

impl SymbolRegistry {
    /// Create a new symbol registry
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            expected_signatures: HashMap::new(),
            required_count: 0,
            resolved_count: 0,
        }
    }

    /// Register an expected symbol signature
    pub fn register_expected(&mut self, signature: FunctionSignature) {
        if !signature.is_optional {
            self.required_count += 1;
        }
        self.expected_signatures
            .insert(signature.name.clone(), signature);
    }

    /// Register multiple expected signatures
    pub fn register_expected_batch(&mut self, signatures: Vec<FunctionSignature>) {
        for sig in signatures {
            self.register_expected(sig);
        }
    }

    /// Record a resolved symbol
    pub fn register_resolved(&mut self, symbol: ResolvedSymbol) -> Result<(), SymbolRegistryError> {
        // Check if we have an expected signature for this symbol
        if let Some(expected_sig) = self.expected_signatures.get(&symbol.name) {
            // Validate the signature matches
            expected_sig.validate(&symbol.signature).map_err(|e| {
                SymbolRegistryError::SignatureValidationFailed {
                    symbol: symbol.name.clone(),
                    error: format!("{}", e),
                }
            })?;
        } else if symbol.is_required {
            // Required symbol but no expected signature registered
            return Err(SymbolRegistryError::UnexpectedSymbol {
                symbol: symbol.name.clone(),
            });
        }

        if symbol.is_resolved {
            self.resolved_count += 1;
        }

        self.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    /// Check if all required symbols are resolved
    pub fn is_complete(&self) -> bool {
        self.resolved_count >= self.required_count
    }

    /// Get resolution status
    pub fn resolution_status(&self) -> SymbolResolutionStatus {
        SymbolResolutionStatus {
            total_expected: self.expected_signatures.len(),
            required_symbols: self.required_count,
            resolved_symbols: self.resolved_count,
            is_complete: self.is_complete(),
        }
    }

    /// Get a resolved symbol by name
    pub fn get(&self, name: &str) -> Option<&ResolvedSymbol> {
        self.symbols.get(name)
    }

    /// Get all resolved symbols
    pub fn all_symbols(&self) -> Vec<&ResolvedSymbol> {
        self.symbols.values().collect()
    }

    /// List all unresolved required symbols
    pub fn unresolved_symbols(&self) -> Vec<String> {
        self.expected_signatures
            .values()
            .filter(|sig| {
                !sig.is_optional
                    && self
                        .symbols
                        .get(&sig.name)
                        .map(|s| !s.is_resolved)
                        .unwrap_or(true)
            })
            .map(|sig| sig.name.clone())
            .collect()
    }

    /// List all missing symbols (not found in plugin)
    pub fn missing_symbols(&self) -> Vec<String> {
        self.expected_signatures
            .values()
            .filter(|sig| !sig.is_optional && !self.symbols.contains_key(&sig.name))
            .map(|sig| sig.name.clone())
            .collect()
    }
}

impl Default for SymbolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Status information about symbol resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolResolutionStatus {
    /// Total number of expected symbols
    pub total_expected: usize,

    /// Number of required symbols
    pub required_symbols: usize,

    /// Number of actually resolved symbols
    pub resolved_symbols: usize,

    /// Whether resolution is complete
    pub is_complete: bool,
}

impl fmt::Display for SymbolResolutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Symbols: {}/{} resolved ({} required)",
            self.resolved_symbols, self.total_expected, self.required_symbols
        )
    }
}

/// Errors that can occur in symbol registry operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolRegistryError {
    /// Symbol signature doesn't match expected
    SignatureValidationFailed { symbol: String, error: String },

    /// Unexpected symbol found that wasn't registered as expected
    UnexpectedSymbol { symbol: String },

    /// Required symbol is missing
    MissingRequiredSymbol { symbol: String },

    /// Symbol address is null
    NullSymbolAddress { symbol: String },
}

impl fmt::Display for SymbolRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolRegistryError::SignatureValidationFailed { symbol, error } => {
                write!(
                    f,
                    "Symbol '{}' signature validation failed: {}",
                    symbol, error
                )
            }
            SymbolRegistryError::UnexpectedSymbol { symbol } => {
                write!(f, "Unexpected symbol '{}' found", symbol)
            }
            SymbolRegistryError::MissingRequiredSymbol { symbol } => {
                write!(f, "Required symbol '{}' is missing", symbol)
            }
            SymbolRegistryError::NullSymbolAddress { symbol } => {
                write!(f, "Symbol '{}' has null address", symbol)
            }
        }
    }
}

impl std::error::Error for SymbolRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    // FunctionSignature tests
    #[test]
    fn test_function_signature_new() {
        let sig = FunctionSignature::new("test_func", "void");
        assert_eq!(sig.name, "test_func");
        assert_eq!(sig.return_type, "void");
        assert!(sig.params.is_empty());
        assert!(!sig.is_optional);
    }

    #[test]
    fn test_function_signature_with_params() {
        let sig = FunctionSignature::new("add", "int")
            .with_param("int")
            .with_param("int");

        assert_eq!(sig.name, "add");
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0], "int");
        assert_eq!(sig.params[1], "int");
    }

    #[test]
    fn test_function_signature_optional() {
        let sig = FunctionSignature::new("optional_func", "void").optional();
        assert!(sig.is_optional);
    }

    #[test]
    fn test_function_signature_string() {
        let sig = FunctionSignature::new("test", "int")
            .with_param("int")
            .with_param("char*");

        let sig_str = sig.signature_string();
        assert!(sig_str.contains("test"));
        assert!(sig_str.contains("int"));
        assert!(sig_str.contains("char*"));
    }

    #[test]
    fn test_function_signature_validate_match() {
        let sig1 = FunctionSignature::new("func", "int")
            .with_param("int")
            .with_param("int");

        let sig2 = FunctionSignature::new("func", "int")
            .with_param("int")
            .with_param("int");

        assert!(sig1.validate(&sig2).is_ok());
    }

    #[test]
    fn test_function_signature_validate_name_mismatch() {
        let sig1 = FunctionSignature::new("func1", "int");
        let sig2 = FunctionSignature::new("func2", "int");

        let result = sig1.validate(&sig2);
        assert!(matches!(result, Err(SignatureError::NameMismatch { .. })));
    }

    #[test]
    fn test_function_signature_validate_return_type_mismatch() {
        let sig1 = FunctionSignature::new("func", "int");
        let sig2 = FunctionSignature::new("func", "void");

        let result = sig1.validate(&sig2);
        assert!(matches!(
            result,
            Err(SignatureError::ReturnTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_function_signature_validate_param_count_mismatch() {
        let sig1 = FunctionSignature::new("func", "int").with_param("int");

        let sig2 = FunctionSignature::new("func", "int")
            .with_param("int")
            .with_param("int");

        let result = sig1.validate(&sig2);
        assert!(matches!(
            result,
            Err(SignatureError::ParameterCountMismatch { .. })
        ));
    }

    #[test]
    fn test_function_signature_validate_param_type_mismatch() {
        let sig1 = FunctionSignature::new("func", "int")
            .with_param("int")
            .with_param("char*");

        let sig2 = FunctionSignature::new("func", "int")
            .with_param("int")
            .with_param("void*");

        let result = sig1.validate(&sig2);
        assert!(matches!(
            result,
            Err(SignatureError::ParameterTypeMismatch { param_index: 1, .. })
        ));
    }

    #[test]
    fn test_signature_error_display() {
        let err = SignatureError::NameMismatch {
            expected: "func1".to_string(),
            found: "func2".to_string(),
        };
        assert!(err.to_string().contains("func1"));
        assert!(err.to_string().contains("func2"));
    }

    // ResolvedSymbol tests
    #[test]
    fn test_resolved_symbol_new() {
        let sig = FunctionSignature::new("test", "void");
        let symbol = ResolvedSymbol::new("test", sig.clone(), 0x1000);

        assert_eq!(symbol.name, "test");
        assert_eq!(symbol.address, 0x1000);
        assert!(symbol.is_required);
        assert!(symbol.is_resolved);
    }

    #[test]
    fn test_resolved_symbol_optional() {
        let sig = FunctionSignature::new("test", "void");
        let symbol = ResolvedSymbol::new("test", sig, 0x1000).optional();

        assert!(!symbol.is_required);
    }

    #[test]
    fn test_resolved_symbol_unresolved() {
        let sig = FunctionSignature::new("test", "void");
        let symbol = ResolvedSymbol::unresolved("test", sig);

        assert_eq!(symbol.address, 0);
        assert!(!symbol.is_resolved);
    }

    // SymbolRegistry tests
    #[test]
    fn test_symbol_registry_new() {
        let registry = SymbolRegistry::new();
        assert_eq!(registry.required_count, 0);
        assert_eq!(registry.resolved_count, 0);
    }

    #[test]
    fn test_symbol_registry_register_expected() {
        let mut registry = SymbolRegistry::new();
        let sig = FunctionSignature::new("test", "void");

        registry.register_expected(sig);
        assert_eq!(registry.required_count, 1);
    }

    #[test]
    fn test_symbol_registry_register_expected_optional() {
        let mut registry = SymbolRegistry::new();
        let sig = FunctionSignature::new("test", "void").optional();

        registry.register_expected(sig);
        assert_eq!(registry.required_count, 0);
    }

    #[test]
    fn test_symbol_registry_register_resolved() {
        let mut registry = SymbolRegistry::new();
        let sig = FunctionSignature::new("test", "void");

        registry.register_expected(sig.clone());

        let symbol = ResolvedSymbol::new("test", sig, 0x1000);
        assert!(registry.register_resolved(symbol).is_ok());
        assert_eq!(registry.resolved_count, 1);
    }

    #[test]
    fn test_symbol_registry_is_complete() {
        let mut registry = SymbolRegistry::new();
        let sig = FunctionSignature::new("test", "void");

        registry.register_expected(sig.clone());
        assert!(!registry.is_complete());

        let symbol = ResolvedSymbol::new("test", sig, 0x1000);
        let _ = registry.register_resolved(symbol);
        assert!(registry.is_complete());
    }

    #[test]
    fn test_symbol_registry_get() {
        let mut registry = SymbolRegistry::new();
        let sig = FunctionSignature::new("test", "void");

        registry.register_expected(sig.clone());
        let symbol = ResolvedSymbol::new("test", sig, 0x1000);
        let _ = registry.register_resolved(symbol);

        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_symbol_registry_resolution_status() {
        let mut registry = SymbolRegistry::new();
        let sig = FunctionSignature::new("test", "void");

        registry.register_expected(sig.clone());
        let status = registry.resolution_status();

        assert_eq!(status.required_symbols, 1);
        assert_eq!(status.resolved_symbols, 0);
        assert!(!status.is_complete);
    }

    #[test]
    fn test_symbol_registry_missing_symbols() {
        let mut registry = SymbolRegistry::new();
        let sig1 = FunctionSignature::new("test1", "void");
        let sig2 = FunctionSignature::new("test2", "void");

        registry.register_expected(sig1);
        registry.register_expected(sig2.clone());

        let symbol = ResolvedSymbol::new("test1", FunctionSignature::new("test1", "void"), 0x1000);
        let _ = registry.register_resolved(symbol);

        let missing = registry.missing_symbols();
        assert_eq!(missing.len(), 1);
        assert!(missing.contains(&"test2".to_string()));
    }

    #[test]
    fn test_symbol_registry_unresolved_symbols() {
        let mut registry = SymbolRegistry::new();
        let sig1 = FunctionSignature::new("test1", "void");
        let sig2 = FunctionSignature::new("test2", "void");

        registry.register_expected(sig1.clone());
        registry.register_expected(sig2);

        let symbol = ResolvedSymbol::unresolved("test1", sig1);
        let _ = registry.register_resolved(symbol);

        let unresolved = registry.unresolved_symbols();
        assert!(unresolved.len() >= 1);
    }

    #[test]
    fn test_symbol_resolution_status_display() {
        let status = SymbolResolutionStatus {
            total_expected: 5,
            required_symbols: 3,
            resolved_symbols: 2,
            is_complete: false,
        };

        let msg = status.to_string();
        assert!(msg.contains("2"));
        assert!(msg.contains("5"));
        assert!(msg.contains("3"));
    }

    #[test]
    fn test_symbol_registry_error_display() {
        let err = SymbolRegistryError::MissingRequiredSymbol {
            symbol: "test".to_string(),
        };
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_signature_error_is_error_trait() {
        use std::error::Error;
        let err: Box<dyn Error> = Box::new(SignatureError::NameMismatch {
            expected: "a".to_string(),
            found: "b".to_string(),
        });
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_symbol_registry_error_is_error_trait() {
        use std::error::Error;
        let err: Box<dyn Error> = Box::new(SymbolRegistryError::MissingRequiredSymbol {
            symbol: "test".to_string(),
        });
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_symbol_registry_batch_registration() {
        let mut registry = SymbolRegistry::new();
        let sigs = vec![
            FunctionSignature::new("func1", "void"),
            FunctionSignature::new("func2", "int"),
            FunctionSignature::new("func3", "char*"),
        ];

        registry.register_expected_batch(sigs);
        assert_eq!(registry.required_count, 3);
    }

    #[test]
    fn test_symbol_registry_all_symbols() {
        let mut registry = SymbolRegistry::new();
        let sig1 = FunctionSignature::new("test1", "void");
        let sig2 = FunctionSignature::new("test2", "void");

        registry.register_expected(sig1.clone());
        registry.register_expected(sig2.clone());

        let symbol1 = ResolvedSymbol::new("test1", sig1, 0x1000);
        let symbol2 = ResolvedSymbol::new("test2", sig2, 0x2000);

        let _ = registry.register_resolved(symbol1);
        let _ = registry.register_resolved(symbol2);

        let all = registry.all_symbols();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_function_signature_display() {
        let sig = FunctionSignature::new("test", "int")
            .with_param("int")
            .with_param("char*");

        let display = format!("{}", sig);
        assert!(display.contains("test"));
        assert!(display.contains("int"));
    }
}
