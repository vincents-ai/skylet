// Kubernetes client adapters for different implementations
// Provides mock and real implementations for Kubernetes cluster management
use super::*;
use async_trait::async_trait;
use serde_json::json;

// Mock Kubernetes client for testing
pub struct MockKubernetesClient {
    config: Option<KubernetesConfig>,
    pods: std::sync::Arc<tokio::sync::RwLock<Vec<Pod>>>,
    deployments: std::sync::Arc<tokio::sync::RwLock<Vec<Deployment>>>,
    services: std::sync::Arc<tokio::sync::RwLock<Vec<Service>>>,
    configmaps: std::sync::Arc<tokio::sync::RwLock<Vec<ConfigMap>>>,
    secrets: std::sync::Arc<tokio::sync::RwLock<Vec<Secret>>>,
    nodes: std::sync::Arc<tokio::sync::RwLock<Vec<Node>>>,
}

impl MockKubernetesClient {
    pub fn new() -> Self {
        Self {
            config: None,
            pods: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
            deployments: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
            services: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
            configmaps: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
            secrets: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
            nodes: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    async fn initialize_mock_data(&self) {
        // Add some mock nodes
        let mut nodes = self.nodes.write().await;
        nodes.push(Node {
            api_version: "v1".to_string(),
            kind: "Node".to_string(),
            metadata: KubernetesMetadata {
                name: "node-1".to_string(),
                namespace: None,
                labels: Some(HashMap::from([
                    ("kubernetes.io/hostname".to_string(), "node-1".to_string()),
                ])),
                annotations: None,
                creation_timestamp: Some(Utc::now()),
                generation: None,
                resource_version: None,
                uid: None,
            },
            spec: Some(NodeSpec {
                pod_cid_r: Some("10.244.0.0/24".to_string()),
                provider_id: None,
                unschedulable: Some(false),
            }),
            status: Some(NodeStatus {
                capacity: Some(HashMap::from([
                    ("cpu".to_string(), "2".to_string()),
                    ("memory".to_string(), "4Gi".to_string()),
                    ("pods".to_string(), "110".to_string()),
                ])),
                allocatable: Some(HashMap::from([
                    ("cpu".to_string(), "2".to_string()),
                    ("memory".to_string(), "4Gi".to_string()),
                    ("pods".to_string(), "110".to_string()),
                ])),
                conditions: Some(vec![NodeCondition {
                    type_: "Ready".to_string(),
                    status: "True".to_string(),
                    last_heartbeat_time: Some(Utc::now()),
                    last_transition_time: Some(Utc::now()),
                    reason: Some("KubeletReady".to_string()),
                    message: Some("kubelet is posting ready status".to_string()),
                }]),
                addresses: Some(vec![NodeAddress {
                    type_: "InternalIP".to_string(),
                    address: "192.168.1.10".to_string(),
                }]),
                node_info: Some(NodeSystemInfo {
                    machine_id: Some("1234567890abcdef".to_string()),
                    system_uuid: Some("12345678-1234-1234-1234-123456789abc".to_string()),
                    boot_id: Some("abcdef123456".to_string()),
                    kernel_version: Some("5.4.0".to_string()),
                    os_image: Some("Ubuntu 20.04 LTS".to_string()),
                    container_runtime_version: Some("containerd://1.4.0".to_string()),
                    kubelet_version: Some("v1.20.0".to_string()),
                    kube_proxy_version: Some("v1.20.0".to_string()),
                    operating_system: Some("linux".to_string()),
                    architecture: Some("amd64".to_string()),
                }),
            }),
        });
    }
}

#[async_trait]
impl KubernetesClient for MockKubernetesClient {
    async fn initialize(&mut self, config: KubernetesConfig) -> Result<()> {
        self.config = Some(config);
        self.initialize_mock_data().await;
        Ok(())
    }

    async fn test_connection(&self) -> Result<bool> {
        // Mock connection test
        Ok(self.config.is_some())
    }

    async fn get_cluster_info(&self) -> Result<ClusterInfo> {
        Ok(ClusterInfo {
            version: "1.20.0".to_string(),
            server_version: KubeVersion {
                major: "1".to_string(),
                minor: "20".to_string(),
                git_version: "v1.20.0".to_string(),
                git_commit: Some("58fa559d052d8196bf65a0b653d5b0ff1af74b".to_string()),
                git_tree_state: Some("clean".to_string()),
                build_date: Some("2020-12-08T14:10:51Z".to_string()),
                go_version: Some("go1.15.5".to_string()),
                compiler: Some("gc".to_string()),
                platform: Some("linux/amd64".to_string()),
            },
            kubernetes_version: KubeVersion {
                major: "1".to_string(),
                minor: "20".to_string(),
                git_version: "v1.20.0".to_string(),
                git_commit: Some("58fa559d052d8196bf65a0b653d5b0ff1af74b".to_string()),
                git_tree_state: Some("clean".to_string()),
                build_date: Some("2020-12-08T14:10:51Z".to_string()),
                go_version: Some("go1.15.5".to_string()),
                compiler: Some("gc".to_string()),
                platform: Some("linux/amd64".to_string()),
            },
        })
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        Ok(vec![
            "default".to_string(),
            "kube-system".to_string(),
            "kube-public".to_string(),
            "kube-node-lease".to_string(),
        ])
    }

    async fn get_nodes(&self) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.clone())
    }

    async fn get_pods(&self, _namespace: &str) -> Result<Vec<Pod>> {
        let pods = self.pods.read().await;
        Ok(pods.clone())
    }

    async fn get_pod(&self, _namespace: &str, name: &str) -> Result<Option<Pod>> {
        let pods = self.pods.read().await;
        Ok(pods.iter().find(|p| p.metadata.name == name).cloned())
    }

    async fn create_pod(&self, _namespace: &str, pod: &Pod) -> Result<Pod> {
        let mut pods = self.pods.write().await;
        let mut new_pod = pod.clone();
        new_pod.metadata.creation_timestamp = Some(Utc::now());
        pods.push(new_pod.clone());
        Ok(new_pod)
    }

    async fn update_pod(&self, _namespace: &str, name: &str, pod: &Pod) -> Result<Pod> {
        let mut pods = self.pods.write().await;
        if let Some(index) = pods.iter().position(|p| p.metadata.name == name) {
            let mut updated_pod = pod.clone();
            updated_pod.metadata.name = name.to_string();
            pods[index] = updated_pod.clone();
            return Ok(updated_pod);
        }
        Err(KubernetesError::not_found(format!("pod {} not found", name)).into())
    }

    async fn delete_pod(&self, _namespace: &str, name: &str) -> Result<()> {
        let mut pods = self.pods.write().await;
        pods.retain(|p| p.metadata.name != name);
        Ok(())
    }

    async fn get_pod_logs(&self, _namespace: &str, name: &str) -> Result<String> {
        // Mock logs
        Ok(format!("Mock logs for pod: {}", name))
    }

    async fn exec_in_pod(&self, _namespace: &str, name: &str, command: &str) -> Result<String> {
        // Mock exec result
        Ok(format!("Mock exec result for pod {} with command: {}", name, command))
    }

    async fn get_deployments(&self, _namespace: &str) -> Result<Vec<Deployment>> {
        let deployments = self.deployments.read().await;
        Ok(deployments.clone())
    }

    async fn get_deployment(&self, _namespace: &str, name: &str) -> Result<Option<Deployment>> {
        let deployments = self.deployments.read().await;
        Ok(deployments.iter().find(|d| d.metadata.name == name).cloned())
    }

    async fn create_deployment(&self, _namespace: &str, deployment: &Deployment) -> Result<Deployment> {
        let mut deployments = self.deployments.write().await;
        let mut new_deployment = deployment.clone();
        new_deployment.metadata.creation_timestamp = Some(Utc::now());
        deployments.push(new_deployment.clone());
        Ok(new_deployment)
    }

    async fn update_deployment(&self, _namespace: &str, name: &str, deployment: &Deployment) -> Result<Deployment> {
        let mut deployments = self.deployments.write().await;
        if let Some(index) = deployments.iter().position(|d| d.metadata.name == name) {
            let mut updated_deployment = deployment.clone();
            updated_deployment.metadata.name = name.to_string();
            deployments[index] = updated_deployment.clone();
            return Ok(updated_deployment);
        }
        Err(KubernetesError::not_found(format!("deployment {} not found", name)).into())
    }

    async fn delete_deployment(&self, _namespace: &str, name: &str) -> Result<()> {
        let mut deployments = self.deployments.write().await;
        deployments.retain(|d| d.metadata.name != name);
        Ok(())
    }

    async fn scale_deployment(&self, namespace: &str, name: &str, replicas: i32) -> Result<Deployment> {
        let deployments = self.deployments.read().await;
        if let Some(deployment) = deployments.iter().find(|d| d.metadata.name == name) {
            let mut updated_deployment = deployment.clone();
            if let Some(ref mut spec) = updated_deployment.spec {
                spec.replicas = Some(replicas);
            }
            drop(deployments);
            
            return self.update_deployment(namespace, name, &updated_deployment).await;
        }
        Err(KubernetesError::not_found(format!("deployment {} not found", name)).into())
    }

    async fn get_services(&self, _namespace: &str) -> Result<Vec<Service>> {
        let services = self.services.read().await;
        Ok(services.clone())
    }

    async fn get_service(&self, _namespace: &str, name: &str) -> Result<Option<Service>> {
        let services = self.services.read().await;
        Ok(services.iter().find(|s| s.metadata.name == name).cloned())
    }

    async fn create_service(&self, _namespace: &str, service: &Service) -> Result<Service> {
        let mut services = self.services.write().await;
        let mut new_service = service.clone();
        new_service.metadata.creation_timestamp = Some(Utc::now());
        services.push(new_service.clone());
        Ok(new_service)
    }

    async fn update_service(&self, _namespace: &str, name: &str, service: &Service) -> Result<Service> {
        let mut services = self.services.write().await;
        if let Some(index) = services.iter().position(|s| s.metadata.name == name) {
            let mut updated_service = service.clone();
            updated_service.metadata.name = name.to_string();
            services[index] = updated_service.clone();
            return Ok(updated_service);
        }
        Err(KubernetesError::not_found(format!("service {} not found", name)).into())
    }

    async fn delete_service(&self, _namespace: &str, name: &str) -> Result<()> {
        let mut services = self.services.write().await;
        services.retain(|s| s.metadata.name != name);
        Ok(())
    }

    async fn get_configmaps(&self, _namespace: &str) -> Result<Vec<ConfigMap>> {
        let configmaps = self.configmaps.read().await;
        Ok(configmaps.clone())
    }

    async fn get_configmap(&self, _namespace: &str, name: &str) -> Result<Option<ConfigMap>> {
        let configmaps = self.configmaps.read().await;
        Ok(configmaps.iter().find(|c| c.metadata.name == name).cloned())
    }

    async fn create_configmap(&self, _namespace: &str, configmap: &ConfigMap) -> Result<ConfigMap> {
        let mut configmaps = self.configmaps.write().await;
        let mut new_configmap = configmap.clone();
        new_configmap.metadata.creation_timestamp = Some(Utc::now());
        configmaps.push(new_configmap.clone());
        Ok(new_configmap)
    }

    async fn update_configmap(&self, _namespace: &str, name: &str, configmap: &ConfigMap) -> Result<ConfigMap> {
        let mut configmaps = self.configmaps.write().await;
        if let Some(index) = configmaps.iter().position(|c| c.metadata.name == name) {
            let mut updated_configmap = configmap.clone();
            updated_configmap.metadata.name = name.to_string();
            configmaps[index] = updated_configmap.clone();
            return Ok(updated_configmap);
        }
        Err(KubernetesError::not_found(format!("configmap {} not found", name)).into())
    }

    async fn delete_configmap(&self, _namespace: &str, name: &str) -> Result<()> {
        let mut configmaps = self.configmaps.write().await;
        configmaps.retain(|c| c.metadata.name != name);
        Ok(())
    }

    async fn get_secrets(&self, _namespace: &str) -> Result<Vec<Secret>> {
        let secrets = self.secrets.read().await;
        Ok(secrets.clone())
    }

    async fn get_secret(&self, _namespace: &str, name: &str) -> Result<Option<Secret>> {
        let secrets = self.secrets.read().await;
        Ok(secrets.iter().find(|s| s.metadata.name == name).cloned())
    }

    async fn create_secret(&self, _namespace: &str, secret: &Secret) -> Result<Secret> {
        let mut secrets = self.secrets.write().await;
        let mut new_secret = secret.clone();
        new_secret.metadata.creation_timestamp = Some(Utc::now());
        secrets.push(new_secret.clone());
        Ok(new_secret)
    }

    async fn update_secret(&self, _namespace: &str, name: &str, secret: &Secret) -> Result<Secret> {
        let mut secrets = self.secrets.write().await;
        if let Some(index) = secrets.iter().position(|s| s.metadata.name == name) {
            let mut updated_secret = secret.clone();
            updated_secret.metadata.name = name.to_string();
            secrets[index] = updated_secret.clone();
            return Ok(updated_secret);
        }
        Err(KubernetesError::not_found(format!("secret {} not found", name)).into())
    }

    async fn delete_secret(&self, _namespace: &str, name: &str) -> Result<()> {
        let mut secrets = self.secrets.write().await;
        secrets.retain(|s| s.metadata.name != name);
        Ok(())
    }

    async fn apply_resource(&self, _namespace: &str, resource: &KubernetesResource) -> Result<KubernetesResource> {
        // Mock apply - return the resource with updated metadata
        let mut applied_resource = resource.clone();
        applied_resource.metadata.creation_timestamp = Some(Utc::now());
        Ok(applied_resource)
    }

    async fn delete_resource(&self, _namespace: &str, kind: &str, name: &str) -> Result<()> {
        // Mock deletion based on kind
        match kind {
            "pod" => self.delete_pod(_namespace, name).await,
            "deployment" => self.delete_deployment(_namespace, name).await,
            "service" => self.delete_service(_namespace, name).await,
            "configmap" => self.delete_configmap(_namespace, name).await,
            "secret" => self.delete_secret(_namespace, name).await,
            _ => Err(KubernetesError::invalid_resource(format!("Unknown kind: {}", kind)).into()),
        }
    }

    async fn watch_resources(&self, _namespace: &str, _kind: &str) -> Result<WatchStream> {
        // Mock watch stream
        Ok(WatchStream {
            kind: "WatchEvent".to_string(),
            objects: vec![],
        })
    }
}

// HTTP Kubernetes client (placeholder for real implementation)
pub struct HttpKubernetesClient {
    config: Option<KubernetesConfig>,
    base_url: Option<String>,
    token: Option<String>,
}

impl HttpKubernetesClient {
    pub fn new() -> Self {
        Self {
            config: None,
            base_url: None,
            token: None,
        }
    }

    fn make_request(&self, method: &str, path: &str, body: Option<&serde_json::Value>) -> Result<serde_json::Value, KubernetesError> {
        let base = self.base_url.as_ref().ok_or_else(|| KubernetesError::configuration("Base URL not set"))?;
        let token = self.token.as_ref().ok_or_else(|| KubernetesError::configuration("Token not set"))?;
        
        let url = format!("{}{}", base, path);
        let agent = ureq::AgentBuilder::new().build();
        
        let response = match method {
            "GET" => agent.get(&url)
                .set("Authorization", &format!("Bearer {}", token))
                .call()?,
            "POST" => {
                let body = body.ok_or_else(|| KubernetesError::configuration("Request body required"))?;
                agent.post(&url)
                    .set("Authorization", &format!("Bearer {}", token))
                    .set("Content-Type", "application/json")
                    .send_string(&body.to_string())?
            }
            "PUT" => {
                let body = body.ok_or_else(|| KubernetesError::configuration("Request body required"))?;
                agent.put(&url)
                    .set("Authorization", &format!("Bearer {}", token))
                    .set("Content-Type", "application/json")
                    .send_string(&body.to_string())?
            }
            "DELETE" => agent.delete(&url)
                .set("Authorization", &format!("Bearer {}", token))
                .call()?,
            _ => return Err(KubernetesError::configuration(format!("Unsupported method: {}", method))),
        };
        
        let body = response.into_string().map_err(|e| KubernetesError::api(format!("Failed to read response: {}", e)))?;
        serde_json::from_str::<serde_json::Value>(&body).map_err(|e| KubernetesError::api(format!("Failed to parse response: {}", e)))
    }

    fn get_items_array(response: &serde_json::Value) -> Vec<serde_json::Value> {
        response["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }
}

#[async_trait]
impl KubernetesClient for HttpKubernetesClient {
    async fn initialize(&mut self, config: KubernetesConfig) -> Result<()> {
        self.config = Some(config.clone());
        
        // Build base URL from config
        let api_server = config.api_server
            .unwrap_or_else(|| "https://kubernetes.default.svc".to_string());
        
        self.base_url = Some(format!("{}/api/v1", api_server));
        self.token = config.token;
        
        Ok(())
    }

    async fn test_connection(&self) -> Result<bool> {
        match self.make_request("GET", "/version", None) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn get_cluster_info(&self) -> Result<ClusterInfo> {
        // This would be implemented to parse the actual response
        Ok(ClusterInfo {
            version: "1.20.0".to_string(),
            server_version: KubeVersion {
                major: "1".to_string(),
                minor: "20".to_string(),
                git_version: "v1.20.0".to_string(),
                git_commit: Some("58fa559d052d8196bf65a0b653d5b0ff1af74b".to_string()),
                git_tree_state: Some("clean".to_string()),
                build_date: Some("2020-12-08T14:10:51Z".to_string()),
                go_version: Some("go1.15.5".to_string()),
                compiler: Some("gc".to_string()),
                platform: Some("linux/amd64".to_string()),
            },
            kubernetes_version: KubeVersion {
                major: "1".to_string(),
                minor: "20".to_string(),
                git_version: "v1.20.0".to_string(),
                git_commit: Some("58fa559d052d8196bf65a0b653d5b0ff1af74b".to_string()),
                git_tree_state: Some("clean".to_string()),
                build_date: Some("2020-12-08T14:10:51Z".to_string()),
                go_version: Some("go1.15.5".to_string()),
                compiler: Some("gc".to_string()),
                platform: Some("linux/amd64".to_string()),
            },
        })
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        match self.make_request("GET", "/namespaces", None) {
            Ok(response) => {
                let items = Self::get_items_array(&response);
                Ok(items.iter()
                    .filter_map(|item| item["metadata"]["name"].as_str())
                    .map(|s| s.to_string())
                    .collect())
            }
            Err(e) => Err(e.into()),
        }
    }

    // Placeholder implementations - would be fully implemented in a real client
    async fn get_nodes(&self) -> Result<Vec<Node>> {
        match self.make_request("GET", "/nodes", None) {
            Ok(response) => {
                let items = Self::get_items_array(&response);
                let nodes: Result<Vec<Node>, _> = items.iter()
                    .map(|item| serde_json::from_value(item.clone())
                        .map_err(|e| KubernetesError::api(format!("Failed to deserialize node: {}", e)).into()))
                    .collect();
                nodes
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn get_pods(&self, namespace: &str) -> Result<Vec<Pod>> {
        let path = format!("/namespaces/{}/pods", namespace);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let items = Self::get_items_array(&response);
                let pods: Result<Vec<Pod>, _> = items.iter()
                    .map(|item| serde_json::from_value(item.clone())
                        .map_err(|e| KubernetesError::api(format!("Failed to deserialize pod: {}", e)).into()))
                    .collect();
                pods
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn get_pod(&self, namespace: &str, name: &str) -> Result<Option<Pod>> {
        let path = format!("/namespaces/{}/pods/{}", namespace, name);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let pod: Pod = serde_json::from_value(response)
                    .map_err(|e| KubernetesError::api(format!("Failed to deserialize pod: {}", e)))?;
                Ok(Some(pod))
            }
            Err(e) => {
                if e.code == "NOT_FOUND" {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    async fn create_pod(&self, namespace: &str, pod: &Pod) -> Result<Pod> {
        let path = format!("/namespaces/{}/pods", namespace);
        let body = serde_json::to_value(pod)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize pod: {}", e)))?;
        match self.make_request("POST", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize pod response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn update_pod(&self, namespace: &str, name: &str, pod: &Pod) -> Result<Pod> {
        let path = format!("/namespaces/{}/pods/{}", namespace, name);
        let body = serde_json::to_value(pod)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize pod: {}", e)))?;
        match self.make_request("PUT", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize pod response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_pod(&self, namespace: &str, name: &str) -> Result<()> {
        let path = format!("/namespaces/{}/pods/{}", namespace, name);
        self.make_request("DELETE", &path, None)?;
        Ok(())
    }

    async fn get_pod_logs(&self, namespace: &str, name: &str) -> Result<String> {
        let path = format!("/namespaces/{}/pods/{}/log", namespace, name);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                if let Some(log) = response.get("log").and_then(|v| v.as_str()) {
                    Ok(log.to_string())
                } else if let Some(log_str) = response.as_str() {
                    Ok(log_str.to_string())
                } else {
                    Ok(response.to_string())
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn exec_in_pod(&self, _namespace: &str, _name: &str, _command: &str) -> Result<String> {
        // Exec requires WebSocket connection which is more complex
        // For now, return a placeholder response indicating the command execution
        Ok("exec operation requires WebSocket upgrade - not yet fully implemented".to_string())
    }

    async fn get_deployments(&self, namespace: &str) -> Result<Vec<Deployment>> {
        let path = format!("/apis/apps/v1/namespaces/{}/deployments", namespace);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let items = Self::get_items_array(&response);
                let deployments: Result<Vec<Deployment>, _> = items.iter()
                    .map(|item| serde_json::from_value(item.clone())
                        .map_err(|e| KubernetesError::api(format!("Failed to deserialize deployment: {}", e)).into()))
                    .collect();
                deployments
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn get_deployment(&self, namespace: &str, name: &str) -> Result<Option<Deployment>> {
        let path = format!("/apis/apps/v1/namespaces/{}/deployments/{}", namespace, name);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let deployment: Deployment = serde_json::from_value(response)
                    .map_err(|e| KubernetesError::api(format!("Failed to deserialize deployment: {}", e)))?;
                Ok(Some(deployment))
            }
            Err(e) => {
                if e.code == "NOT_FOUND" {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    async fn create_deployment(&self, namespace: &str, deployment: &Deployment) -> Result<Deployment> {
        let path = format!("/apis/apps/v1/namespaces/{}/deployments", namespace);
        let body = serde_json::to_value(deployment)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize deployment: {}", e)))?;
        match self.make_request("POST", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize deployment response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn update_deployment(&self, namespace: &str, name: &str, deployment: &Deployment) -> Result<Deployment> {
        let path = format!("/apis/apps/v1/namespaces/{}/deployments/{}", namespace, name);
        let body = serde_json::to_value(deployment)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize deployment: {}", e)))?;
        match self.make_request("PUT", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize deployment response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_deployment(&self, namespace: &str, name: &str) -> Result<()> {
        let path = format!("/apis/apps/v1/namespaces/{}/deployments/{}", namespace, name);
        self.make_request("DELETE", &path, None)?;
        Ok(())
    }

    async fn scale_deployment(&self, namespace: &str, name: &str, replicas: i32) -> Result<Deployment> {
        // Get current deployment
        if let Some(mut deployment) = self.get_deployment(namespace, name).await? {
            // Update replica count
            if let Some(ref mut spec) = deployment.spec {
                spec.replicas = Some(replicas);
            }
            // Update deployment
            self.update_deployment(namespace, name, &deployment).await
        } else {
            Err(KubernetesError::not_found(format!("Deployment {} not found in namespace {}", name, namespace)).into())
        }
    }

    async fn get_services(&self, namespace: &str) -> Result<Vec<Service>> {
        let path = format!("/namespaces/{}/services", namespace);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let items = Self::get_items_array(&response);
                let services: Result<Vec<Service>, _> = items.iter()
                    .map(|item| serde_json::from_value(item.clone())
                        .map_err(|e| KubernetesError::api(format!("Failed to deserialize service: {}", e)).into()))
                    .collect();
                services
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn get_service(&self, namespace: &str, name: &str) -> Result<Option<Service>> {
        let path = format!("/namespaces/{}/services/{}", namespace, name);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let service: Service = serde_json::from_value(response)
                    .map_err(|e| KubernetesError::api(format!("Failed to deserialize service: {}", e)))?;
                Ok(Some(service))
            }
            Err(e) => {
                if e.code == "NOT_FOUND" {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    async fn create_service(&self, namespace: &str, service: &Service) -> Result<Service> {
        let path = format!("/namespaces/{}/services", namespace);
        let body = serde_json::to_value(service)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize service: {}", e)))?;
        match self.make_request("POST", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize service response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn update_service(&self, namespace: &str, name: &str, service: &Service) -> Result<Service> {
        let path = format!("/namespaces/{}/services/{}", namespace, name);
        let body = serde_json::to_value(service)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize service: {}", e)))?;
        match self.make_request("PUT", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize service response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_service(&self, namespace: &str, name: &str) -> Result<()> {
        let path = format!("/namespaces/{}/services/{}", namespace, name);
        self.make_request("DELETE", &path, None)?;
        Ok(())
    }

    async fn get_configmaps(&self, namespace: &str) -> Result<Vec<ConfigMap>> {
        let path = format!("/namespaces/{}/configmaps", namespace);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let items = Self::get_items_array(&response);
                let configmaps: Result<Vec<ConfigMap>, _> = items.iter()
                    .map(|item| serde_json::from_value(item.clone())
                        .map_err(|e| KubernetesError::api(format!("Failed to deserialize configmap: {}", e)).into()))
                    .collect();
                configmaps
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn get_configmap(&self, namespace: &str, name: &str) -> Result<Option<ConfigMap>> {
        let path = format!("/namespaces/{}/configmaps/{}", namespace, name);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let configmap: ConfigMap = serde_json::from_value(response)
                    .map_err(|e| KubernetesError::api(format!("Failed to deserialize configmap: {}", e)))?;
                Ok(Some(configmap))
            }
            Err(e) => {
                if e.code == "NOT_FOUND" {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    async fn create_configmap(&self, namespace: &str, configmap: &ConfigMap) -> Result<ConfigMap> {
        let path = format!("/namespaces/{}/configmaps", namespace);
        let body = serde_json::to_value(configmap)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize configmap: {}", e)))?;
        match self.make_request("POST", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize configmap response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn update_configmap(&self, namespace: &str, name: &str, configmap: &ConfigMap) -> Result<ConfigMap> {
        let path = format!("/namespaces/{}/configmaps/{}", namespace, name);
        let body = serde_json::to_value(configmap)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize configmap: {}", e)))?;
        match self.make_request("PUT", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize configmap response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_configmap(&self, namespace: &str, name: &str) -> Result<()> {
        let path = format!("/namespaces/{}/configmaps/{}", namespace, name);
        self.make_request("DELETE", &path, None)?;
        Ok(())
    }

    async fn get_secrets(&self, namespace: &str) -> Result<Vec<Secret>> {
        let path = format!("/namespaces/{}/secrets", namespace);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let items = Self::get_items_array(&response);
                let secrets: Result<Vec<Secret>, _> = items.iter()
                    .map(|item| serde_json::from_value(item.clone())
                        .map_err(|e| KubernetesError::api(format!("Failed to deserialize secret: {}", e)).into()))
                    .collect();
                secrets
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn get_secret(&self, namespace: &str, name: &str) -> Result<Option<Secret>> {
        let path = format!("/namespaces/{}/secrets/{}", namespace, name);
        match self.make_request("GET", &path, None) {
            Ok(response) => {
                let secret: Secret = serde_json::from_value(response)
                    .map_err(|e| KubernetesError::api(format!("Failed to deserialize secret: {}", e)))?;
                Ok(Some(secret))
            }
            Err(e) => {
                if e.code == "NOT_FOUND" {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    async fn create_secret(&self, namespace: &str, secret: &Secret) -> Result<Secret> {
        let path = format!("/namespaces/{}/secrets", namespace);
        let body = serde_json::to_value(secret)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize secret: {}", e)))?;
        match self.make_request("POST", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize secret response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn update_secret(&self, namespace: &str, name: &str, secret: &Secret) -> Result<Secret> {
        let path = format!("/namespaces/{}/secrets/{}", namespace, name);
        let body = serde_json::to_value(secret)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize secret: {}", e)))?;
        match self.make_request("PUT", &path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize secret response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_secret(&self, namespace: &str, name: &str) -> Result<()> {
        let path = format!("/namespaces/{}/secrets/{}", namespace, name);
        self.make_request("DELETE", &path, None)?;
        Ok(())
    }

    async fn apply_resource(&self, namespace: &str, resource: &KubernetesResource) -> Result<KubernetesResource> {
        // Determine the appropriate API path based on kind
        let api_path = match resource.kind.as_str() {
            "Deployment" | "DaemonSet" | "StatefulSet" => format!("/apis/apps/v1/namespaces/{}/{}", namespace, resource.kind.to_lowercase()),
            _ => format!("/namespaces/{}/{}", namespace, resource.kind.to_lowercase()),
        };
        
        let body = serde_json::to_value(resource)
            .map_err(|e| KubernetesError::api(format!("Failed to serialize resource: {}", e)))?;
        
        // Apply is essentially a POST/PUT operation
        match self.make_request("POST", &api_path, Some(&body)) {
            Ok(response) => serde_json::from_value(response)
                .map_err(|e| KubernetesError::api(format!("Failed to deserialize resource response: {}", e)).into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_resource(&self, namespace: &str, kind: &str, name: &str) -> Result<()> {
        // Determine the appropriate API path based on kind
        let api_path = match kind {
            "Deployment" | "DaemonSet" | "StatefulSet" => format!("/apis/apps/v1/namespaces/{}/{}/{}", namespace, kind.to_lowercase(), name),
            _ => format!("/namespaces/{}/{}/{}", namespace, kind.to_lowercase(), name),
        };
        
        self.make_request("DELETE", &api_path, None)?;
        Ok(())
    }

    async fn watch_resources(&self, namespace: &str, kind: &str) -> Result<WatchStream> {
        // Determine the appropriate API path based on kind
        let api_path = match kind {
            "Deployment" | "DaemonSet" | "StatefulSet" => format!("/apis/apps/v1/namespaces/{}/{}?watch=true", namespace, kind.to_lowercase()),
            _ => format!("/namespaces/{}/{}?watch=true", namespace, kind.to_lowercase()),
        };
        
        match self.make_request("GET", &api_path, None) {
            Ok(response) => {
                let items = response["items"].as_array().cloned().unwrap_or_default();
                Ok(WatchStream {
                    kind: kind.to_string(),
                    objects: items,
                })
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for MockKubernetesClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for HttpKubernetesClient {
    fn default() -> Self {
        Self::new()
    }
}