// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use skylet_abi::{HttpRequest, HttpResponse};

#[derive(Clone, Debug, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

pub type RouteHandler =
    Arc<dyn Fn(&HttpRequest, &HashMap<String, String>) -> HttpResponse + Send + Sync>;

#[derive(Clone)]
pub struct RouteConfig {
    pub method: HttpMethod,
    pub path: String,
    pub handler: RouteHandler,
    pub description: Option<String>,
}

struct RouteEntry {
    config: RouteConfig,
    regex: Regex,
    param_names: Vec<String>,
}

pub struct HttpRouter {
    routes: RwLock<Vec<RouteEntry>>,
}

static PATH_PARAM_RE: Lazy<Regex> = Lazy::new(|| Regex::new("\\{([a-zA-Z0-9_]+)\\}").unwrap());

impl Default for HttpRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpRouter {
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(Vec::new()),
        }
    }

    pub fn register_route(&self, cfg: RouteConfig) {
        let mut param_names = Vec::new();
        for cap in PATH_PARAM_RE.captures_iter(&cfg.path) {
            param_names.push(cap.get(1).unwrap().as_str().to_string());
        }

        let mut pattern = regex::escape(&cfg.path);
        for name in &param_names {
            let esc = format!("\\{{{}\\}}", name);
            let rep = format!("(?P<{}>[^/]+)", name);
            pattern = pattern.replace(&esc, &rep);
        }
        pattern = format!("^{}$", pattern);
        let regex = Regex::new(&pattern).expect("invalid route regex");

        let entry = RouteEntry {
            config: cfg,
            regex,
            param_names,
        };
        self.routes.write().unwrap().push(entry);
    }

    pub fn find_route(
        &self,
        method: &HttpMethod,
        path: &str,
    ) -> Option<(RouteConfig, HashMap<String, String>)> {
        let routes = self.routes.read().unwrap();
        for entry in routes.iter() {
            if &entry.config.method == method {
                if let Some(caps) = entry.regex.captures(path) {
                    let mut params = HashMap::new();
                    for name in &entry.param_names {
                        if let Some(m) = caps.name(name) {
                            params.insert(name.clone(), m.as_str().to_string());
                        }
                    }
                    return Some((entry.config.clone(), params));
                }
            }
        }
        None
    }

    pub fn generate_openapi(&self) -> serde_json::Value {
        let routes = self.routes.read().unwrap();
        let mut paths = serde_json::Map::new();

        for entry in routes.iter() {
            let p = entry.config.path.clone();
            let method = match entry.config.method {
                HttpMethod::Get => "get",
                HttpMethod::Post => "post",
                HttpMethod::Put => "put",
                HttpMethod::Delete => "delete",
                HttpMethod::Patch => "patch",
            };

            let mut params = Vec::new();
            for name in &entry.param_names {
                params.push(serde_json::json!({
                    "name": name,
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string" }
                }));
            }

            let op = serde_json::json!({
                "summary": entry.config.description.clone().unwrap_or_default(),
                "parameters": params,
                "responses": {"200": {"description": "OK"}}
            });

            let path_item = paths.entry(p).or_insert_with(|| serde_json::json!({}));
            path_item
                .as_object_mut()
                .unwrap()
                .insert(method.to_string(), op);
        }

        serde_json::json!({
            "openapi": "3.0.0",
            "info": {"title": "Plugin Routes", "version": "1.0.0"},
            "paths": paths
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skylet_abi::HttpResponse;
    use std::sync::Arc;

    fn create_test_handler(status: i32) -> RouteHandler {
        Arc::new(move |_req, _params| HttpResponse {
            status_code: status,
            headers: std::ptr::null_mut(),
            num_headers: 0,
            body: std::ptr::null_mut(),
            body_len: 0,
        })
    }

    #[test]
    fn test_register_and_find() {
        let router = HttpRouter::new();

        let handler: RouteHandler = Arc::new(|_req, _params| HttpResponse {
            status_code: 200,
            headers: std::ptr::null_mut(),
            num_headers: 0,
            body: std::ptr::null_mut(),
            body_len: 0,
        });

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/api/items/{id}".to_string(),
            handler: handler.clone(),
            description: Some("get item".to_string()),
        });

        let (cfg, params) = router
            .find_route(&HttpMethod::Get, "/api/items/123")
            .expect("route not found");
        assert_eq!(cfg.path, "/api/items/{id}");
        assert_eq!(params.get("id").unwrap(), "123");
    }

    #[test]
    fn test_router_new() {
        let router = HttpRouter::new();
        let result = router.find_route(&HttpMethod::Get, "/nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_register_single_route() {
        let router = HttpRouter::new();
        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let result = router.find_route(&HttpMethod::Get, "/");
        assert!(result.is_some());
    }

    #[test]
    fn test_register_multiple_routes() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/users".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        router.register_route(RouteConfig {
            method: HttpMethod::Post,
            path: "/users".to_string(),
            handler: create_test_handler(201),
            description: None,
        });

        let get_result = router.find_route(&HttpMethod::Get, "/users");
        assert!(get_result.is_some());

        let post_result = router.find_route(&HttpMethod::Post, "/users");
        assert!(post_result.is_some());
    }

    #[test]
    fn test_path_parameter_extraction() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/users/{user_id}".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let (_, params) = router
            .find_route(&HttpMethod::Get, "/users/42")
            .expect("route not found");

        assert_eq!(params.get("user_id").unwrap(), "42");
    }

    #[test]
    fn test_multiple_path_parameters() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/users/{user_id}/posts/{post_id}".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let (_, params) = router
            .find_route(&HttpMethod::Get, "/users/123/posts/456")
            .expect("route not found");

        assert_eq!(params.get("user_id").unwrap(), "123");
        assert_eq!(params.get("post_id").unwrap(), "456");
    }

    #[test]
    fn test_path_parameter_with_slashes_not_matched() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/files/{filename}".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        // Path parameter shouldn't match paths with slashes
        let result = router.find_route(&HttpMethod::Get, "/files/path/to/file.txt");
        assert!(result.is_none());
    }

    #[test]
    fn test_method_matching() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/resource".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        // GET should match
        assert!(router.find_route(&HttpMethod::Get, "/resource").is_some());

        // Other methods should not match
        assert!(router.find_route(&HttpMethod::Post, "/resource").is_none());
        assert!(router.find_route(&HttpMethod::Put, "/resource").is_none());
        assert!(router
            .find_route(&HttpMethod::Delete, "/resource")
            .is_none());
        assert!(router.find_route(&HttpMethod::Patch, "/resource").is_none());
    }

    #[test]
    fn test_all_http_methods() {
        let router = HttpRouter::new();
        let methods = vec![
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Delete,
            HttpMethod::Patch,
        ];

        for (i, method) in methods.iter().enumerate() {
            router.register_route(RouteConfig {
                method: method.clone(),
                path: "/api/resource".to_string(),
                handler: create_test_handler(200 + i as i32),
                description: None,
            });
        }

        // All methods should be findable
        for method in methods {
            assert!(router.find_route(&method, "/api/resource").is_some());
        }
    }

    #[test]
    fn test_exact_path_matching() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/api/v1/users".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        // Exact match
        assert!(router
            .find_route(&HttpMethod::Get, "/api/v1/users")
            .is_some());

        // No match - extra segment
        assert!(router
            .find_route(&HttpMethod::Get, "/api/v1/users/")
            .is_none());

        // No match - incomplete path
        assert!(router.find_route(&HttpMethod::Get, "/api/v1").is_none());

        // No match - different path
        assert!(router
            .find_route(&HttpMethod::Get, "/api/v2/users")
            .is_none());
    }

    #[test]
    fn test_route_handler_execution() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Post,
            path: "/data".to_string(),
            handler: create_test_handler(201),
            description: None,
        });

        let (config, _params) = router
            .find_route(&HttpMethod::Post, "/data")
            .expect("route not found");

        assert_eq!(config.method, HttpMethod::Post);
        assert_eq!(config.path, "/data");
    }

    #[test]
    fn test_route_with_description() {
        let router = HttpRouter::new();
        let description = "Create a new user".to_string();

        router.register_route(RouteConfig {
            method: HttpMethod::Post,
            path: "/users".to_string(),
            handler: create_test_handler(201),
            description: Some(description.clone()),
        });

        let (config, _) = router
            .find_route(&HttpMethod::Post, "/users")
            .expect("route not found");

        assert_eq!(config.description.unwrap(), description);
    }

    #[test]
    fn test_openapi_generation_single_route() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/items".to_string(),
            handler: create_test_handler(200),
            description: Some("List all items".to_string()),
        });

        let openapi = router.generate_openapi();

        assert_eq!(openapi["openapi"], "3.0.0");
        assert!(openapi["paths"]["/items"].is_object());
        assert!(openapi["paths"]["/items"]["get"].is_object());
    }

    #[test]
    fn test_openapi_generation_multiple_methods() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/items".to_string(),
            handler: create_test_handler(200),
            description: Some("List items".to_string()),
        });

        router.register_route(RouteConfig {
            method: HttpMethod::Post,
            path: "/items".to_string(),
            handler: create_test_handler(201),
            description: Some("Create item".to_string()),
        });

        let openapi = router.generate_openapi();

        assert!(openapi["paths"]["/items"]["get"].is_object());
        assert!(openapi["paths"]["/items"]["post"].is_object());
    }

    #[test]
    fn test_openapi_with_path_parameters() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/items/{item_id}".to_string(),
            handler: create_test_handler(200),
            description: Some("Get item".to_string()),
        });

        let openapi = router.generate_openapi();
        let params = &openapi["paths"]["/items/{item_id}"]["get"]["parameters"];

        assert!(params.is_array());
        assert!(params[0]["name"] == "item_id");
        assert!(params[0]["in"] == "path");
        assert!(params[0]["required"] == true);
    }

    #[test]
    fn test_route_not_found_different_method() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/users".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let result = router.find_route(&HttpMethod::Post, "/users");
        assert!(result.is_none());
    }

    #[test]
    fn test_route_not_found_different_path() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/users".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let result = router.find_route(&HttpMethod::Get, "/posts");
        assert!(result.is_none());
    }

    #[test]
    fn test_parameter_name_preservation() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/api/{resource_id}/sub/{sub_id}".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let (_, params) = router
            .find_route(&HttpMethod::Get, "/api/abc123/sub/xyz789")
            .expect("route not found");

        assert_eq!(params.get("resource_id").unwrap(), "abc123");
        assert_eq!(params.get("sub_id").unwrap(), "xyz789");
    }

    #[test]
    fn test_deep_path_nesting() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/api/v1/users/{user_id}/posts/{post_id}/comments/{comment_id}".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let (_, params) = router
            .find_route(&HttpMethod::Get, "/api/v1/users/1/posts/2/comments/3")
            .expect("route not found");

        assert_eq!(params.len(), 3);
        assert_eq!(params["user_id"], "1");
        assert_eq!(params["post_id"], "2");
        assert_eq!(params["comment_id"], "3");
    }

    #[test]
    fn test_special_characters_in_path() {
        let router = HttpRouter::new();

        router.register_route(RouteConfig {
            method: HttpMethod::Get,
            path: "/files/backup-2024.zip".to_string(),
            handler: create_test_handler(200),
            description: None,
        });

        let result = router.find_route(&HttpMethod::Get, "/files/backup-2024.zip");
        assert!(result.is_some());
    }

    #[test]
    fn test_method_enum_equality() {
        assert_eq!(HttpMethod::Get, HttpMethod::Get);
        assert_ne!(HttpMethod::Get, HttpMethod::Post);
    }
}
