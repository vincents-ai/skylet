// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! RFC-0019: Plugin-Provided API Endpoints
//!
//! This module provides types for dynamic HTTP route registration,
//! allowing plugins to expose their own REST API endpoints.

use std::ffi::{c_char, c_int, c_void};
use std::fmt;

use regex_lite::Regex;

use super::{HttpRequest, HttpResponse, PluginContext, PluginResult};

/// HTTP methods supported by the router.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get = 0,
    Post = 1,
    Put = 2,
    Delete = 3,
    Patch = 4,
    Head = 5,
    Options = 6,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Patch => write!(f, "PATCH"),
            HttpMethod::Head => write!(f, "HEAD"),
            HttpMethod::Options => write!(f, "OPTIONS"),
        }
    }
}

impl HttpMethod {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(HttpMethod::Get),
            "POST" => Some(HttpMethod::Post),
            "PUT" => Some(HttpMethod::Put),
            "DELETE" => Some(HttpMethod::Delete),
            "PATCH" => Some(HttpMethod::Patch),
            "HEAD" => Some(HttpMethod::Head),
            "OPTIONS" => Some(HttpMethod::Options),
            _ => None,
        }
    }
}

/// Route handler function signature.
///
/// This is called by the HTTP router when a matching request is received.
/// The handler receives the request, parsed path parameters (as JSON), and must
/// populate the response.
pub type RouteHandlerFn = extern "C" fn(
    context: *const PluginContext,
    request: *const HttpRequest,
    path_params_json: *const c_char,
    response: *mut *mut HttpResponse,
) -> PluginResult;

/// Configuration for a single API route.
#[repr(C)]
pub struct RouteConfig {
    /// The HTTP method for this route
    pub method: HttpMethod,
    /// The URL path pattern (e.g., "/api/items/{id}")
    /// Supports path parameters in curly braces
    pub path: *const c_char,
    /// Handler function for this route
    pub handler: RouteHandlerFn,
    /// Optional description for API documentation
    pub description: *const c_char,
    /// Optional plugin name (set by router during registration)
    pub plugin_name: *const c_char,
    /// User data passed to handler
    pub user_data: *mut c_void,
}

/// Middleware function signature.
///
/// Middleware functions are called in a chain before the route handler.
/// They can modify the request, response, or short-circuit the chain.
pub type MiddlewareFn = extern "C" fn(
    context: *const PluginContext,
    request: *const HttpRequest,
    response: *mut *mut HttpResponse,
    next: extern "C" fn() -> c_int,
) -> PluginResult;

/// Configuration for a middleware component.
#[repr(C)]
pub struct MiddlewareConfig {
    /// Name of the middleware
    pub name: *const c_char,
    /// The middleware function
    pub handler: MiddlewareFn,
    /// Priority (lower = earlier in chain)
    pub priority: i32,
    /// Whether this middleware is enabled
    pub enabled: bool,
    /// Routes to apply to (null = all routes, comma-separated patterns)
    pub route_patterns: *const c_char,
}

/// HttpRouter service provided by the http-server plugin.
///
/// This is the central registry for all plugin-provided HTTP routes.
#[repr(C)]
pub struct HttpRouter {
    /// Register a new route
    pub register_route:
        extern "C" fn(context: *const PluginContext, config: *const RouteConfig) -> PluginResult,

    /// Unregister a route
    pub unregister_route: extern "C" fn(
        context: *const PluginContext,
        method: HttpMethod,
        path: *const c_char,
    ) -> PluginResult,

    /// Register middleware
    pub register_middleware: extern "C" fn(
        context: *const PluginContext,
        config: *const MiddlewareConfig,
    ) -> PluginResult,

    /// Get all registered routes (returns JSON array)
    pub get_routes: extern "C" fn(context: *const PluginContext) -> *mut c_char,

    /// Generate OpenAPI specification (returns JSON)
    pub get_openapi_spec: extern "C" fn(context: *const PluginContext) -> *mut c_char,
}

// ============================================================================
// RFC-0019: HttpRouterV2 for ABI v2
// ============================================================================

/// Route configuration for ABI v2 plugins.
///
/// Similar to RouteConfig but uses PluginResultV2 for compatibility with V2 ABI.
#[repr(C)]
pub struct RouteConfigV2 {
    /// The HTTP method for this route
    pub method: HttpMethod,
    /// The URL path pattern (e.g., "/api/items/{id}")
    /// Supports path parameters in curly braces
    pub path: *const c_char,
    /// Handler function for this route
    pub handler: RouteHandlerFn,
    /// Optional description for API documentation
    pub description: *const c_char,
    /// Optional plugin name (set by router during registration)
    pub plugin_name: *const c_char,
    /// User data passed to handler
    pub user_data: *mut c_void,
}

/// HttpRouterV2 service for ABI v2 plugins.
///
/// This is the central registry for all plugin-provided HTTP routes,
/// using PluginResultV2 for V2 ABI compatibility.
///
/// # Example
///
/// ```c
/// // In a plugin's plugin_init_v2:
/// RouteConfigV2 route = {
///     .method = HttpMethod_Get,
///     .path = "/api/my-plugin/items/{id}",
///     .handler = my_handler,
///     .description = "Get an item by ID",
/// };
/// context->http_router->register_route(context, &route);
/// ```
#[repr(C)]
pub struct HttpRouterV2 {
    /// Register a new route
    pub register_route: extern "C" fn(
        context: *const crate::v2_spec::PluginContextV2,
        config: *const RouteConfigV2,
    ) -> crate::v2_spec::PluginResultV2,

    /// Unregister a route
    pub unregister_route: extern "C" fn(
        context: *const crate::v2_spec::PluginContextV2,
        method: HttpMethod,
        path: *const c_char,
    ) -> crate::v2_spec::PluginResultV2,

    /// Register middleware
    pub register_middleware: extern "C" fn(
        context: *const crate::v2_spec::PluginContextV2,
        config: *const MiddlewareConfigV2,
    ) -> crate::v2_spec::PluginResultV2,

    /// Get all registered routes (returns JSON array, caller must free with free_string)
    pub get_routes: extern "C" fn(context: *const crate::v2_spec::PluginContextV2) -> *mut c_char,

    /// Generate OpenAPI specification (returns JSON, caller must free with free_string)
    pub get_openapi_spec:
        extern "C" fn(context: *const crate::v2_spec::PluginContextV2) -> *mut c_char,

    /// Free a string returned by get_routes or get_openapi_spec
    pub free_string: extern "C" fn(ptr: *mut c_char),
}

/// Middleware configuration for ABI v2.
#[repr(C)]
pub struct MiddlewareConfigV2 {
    /// Name of the middleware
    pub name: *const c_char,
    /// The middleware function
    pub handler: MiddlewareFnV2,
    /// Priority (lower = earlier in chain)
    pub priority: i32,
    /// Whether this middleware is enabled
    pub enabled: bool,
    /// Routes to apply to (null = all routes, comma-separated patterns)
    pub route_patterns: *const c_char,
}

/// Middleware function signature for V2 ABI.
///
/// Similar to MiddlewareFn but returns PluginResultV2.
pub type MiddlewareFnV2 = extern "C" fn(
    context: *const crate::v2_spec::PluginContextV2,
    request: *const HttpRequest,
    response: *mut *mut HttpResponse,
    next: extern "C" fn() -> c_int,
) -> crate::v2_spec::PluginResultV2;

/// Route metadata for API documentation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RouteMetadata {
    /// HTTP method
    pub method: String,
    /// Path pattern
    pub path: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Plugin that registered this route
    pub plugin: String,
    /// Path parameter names extracted from path
    pub path_params: Vec<String>,
    /// Tags for grouping in documentation
    pub tags: Vec<String>,
}

/// OpenAPI operation specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiOperation {
    /// Operation summary
    pub summary: Option<String>,
    /// Operation description
    pub description: Option<String>,
    /// Operation ID (unique identifier)
    pub operation_id: String,
    /// Tags for grouping
    pub tags: Vec<String>,
    /// Parameters (path, query, header)
    pub parameters: Vec<OpenApiParameter>,
    /// Request body specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<OpenApiRequestBody>,
    /// Response specifications
    pub responses: std::collections::HashMap<String, OpenApiResponse>,
}

/// OpenAPI parameter specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiParameter {
    /// Parameter name
    pub name: String,
    /// Parameter location (path, query, header)
    pub location: String,
    /// Parameter description
    pub description: Option<String>,
    /// Whether parameter is required
    pub required: bool,
    /// Parameter schema
    pub schema: OpenApiSchema,
}

/// OpenAPI request body specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiRequestBody {
    /// Description
    pub description: Option<String>,
    /// Whether body is required
    pub required: bool,
    /// Content type to schema mapping
    pub content: std::collections::HashMap<String, OpenApiMediaType>,
}

/// OpenAPI media type specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiMediaType {
    /// Schema for the content
    pub schema: OpenApiSchema,
}

/// OpenAPI response specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiResponse {
    /// Response description
    pub description: String,
    /// Content type to schema mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<std::collections::HashMap<String, OpenApiMediaType>>,
}

/// OpenAPI schema (simplified).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiSchema {
    /// Schema type
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// For objects: property schemas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<std::collections::HashMap<String, OpenApiSchema>>,
    /// For arrays: item schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<OpenApiSchema>>,
    /// Format hint (e.g., "int64", "date-time")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Full OpenAPI document.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiDocument {
    /// OpenAPI version
    pub openapi: String,
    /// API info
    pub info: OpenApiInfo,
    /// API paths
    pub paths:
        std::collections::HashMap<String, std::collections::HashMap<String, OpenApiOperation>>,
    /// Components (reusable schemas)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<OpenApiComponents>,
}

/// OpenAPI info section.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiInfo {
    /// API title
    pub title: String,
    /// API version
    pub version: String,
    /// API description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// OpenAPI components section.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenApiComponents {
    /// Reusable schemas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schemas: Option<std::collections::HashMap<String, OpenApiSchema>>,
}

/// Extract path parameter names from a path pattern.
///
/// # Example
/// ```
/// use skylet_abi::http::extract_path_params;
///
/// let params = extract_path_params("/api/items/{id}/comments/{comment_id}");
/// assert_eq!(params, vec!["id", "comment_id"]);
/// ```
pub fn extract_path_params(path: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut in_param = false;
    let mut current_param = String::new();

    for ch in path.chars() {
        match ch {
            '{' => {
                in_param = true;
                current_param.clear();
            }
            '}' => {
                if in_param && !current_param.is_empty() {
                    params.push(current_param.clone());
                }
                in_param = false;
            }
            _ if in_param => {
                current_param.push(ch);
            }
            _ => {}
        }
    }

    params
}

/// Convert path pattern to regex for matching.
///
/// # Example
/// ```
/// use skylet_abi::http::path_pattern_to_regex;
///
/// let re = path_pattern_to_regex("/api/items/{id}");
/// assert!(re.is_match("/api/items/123"));
/// assert!(re.is_match("/api/items/abc-xyz"));
/// assert!(!re.is_match("/api/items"));
/// assert!(!re.is_match("/api/items/123/extra"));
/// ```
pub fn path_pattern_to_regex(pattern: &str) -> Regex {
    let mut regex_str = String::new();
    regex_str.push('^');

    let mut in_param = false;
    let mut current_param = String::new();

    for ch in pattern.chars() {
        match ch {
            '{' => {
                in_param = true;
                current_param.clear();
                regex_str.push_str("(?P<");
            }
            '}' => {
                if in_param {
                    regex_str.push_str(&current_param);
                    regex_str.push_str(">[^/]+)");
                }
                in_param = false;
            }
            _ if in_param => {
                current_param.push(ch);
            }
            _ => {
                // Escape regex special characters
                match ch {
                    '.' | '*' | '+' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '|' | '\\' => {
                        regex_str.push('\\');
                        regex_str.push(ch);
                    }
                    _ => {
                        regex_str.push(ch);
                    }
                }
            }
        }
    }

    regex_str.push('$');

    Regex::new(&regex_str).unwrap_or_else(|_| Regex::new("^$").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_path_params() {
        let result: Vec<String> = extract_path_params("/api/items");
        assert_eq!(result, Vec::<String>::new());
        assert_eq!(
            extract_path_params("/api/items/{id}"),
            vec!["id".to_string()]
        );
        assert_eq!(
            extract_path_params("/api/items/{id}/comments/{comment_id}"),
            vec!["id".to_string(), "comment_id".to_string()]
        );
    }

    #[test]
    fn test_path_pattern_to_regex() {
        let re = path_pattern_to_regex("/api/items/{id}");
        assert!(re.is_match("/api/items/123"));
        assert!(re.is_match("/api/items/abc-xyz"));
        assert!(!re.is_match("/api/items"));
        assert!(!re.is_match("/api/items/123/extra"));

        // Test captures
        let caps = re.captures("/api/items/123").unwrap();
        assert_eq!(caps.name("id").unwrap().as_str(), "123");
    }

    #[test]
    fn test_http_method_display() {
        assert_eq!(format!("{}", HttpMethod::Get), "GET");
        assert_eq!(format!("{}", HttpMethod::Post), "POST");
    }

    #[test]
    fn test_http_method_from_str() {
        assert_eq!(HttpMethod::from_str("GET"), Some(HttpMethod::Get));
        assert_eq!(HttpMethod::from_str("get"), Some(HttpMethod::Get));
        assert_eq!(HttpMethod::from_str("Post"), Some(HttpMethod::Post));
        assert_eq!(HttpMethod::from_str("invalid"), None);
    }
}
