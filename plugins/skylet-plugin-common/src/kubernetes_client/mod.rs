// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod adapters;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesError {
    pub code: String,
    pub message: String,
    pub kind: String,
}

impl KubernetesError {
    pub fn api(message: impl Into<String>) -> Self {
        Self {
            code: "API_ERROR".to_string(),
            message: message.into(),
            kind: "error".to_string(),
        }
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self {
            code: "CONFIG_ERROR".to_string(),
            message: message.into(),
            kind: "error".to_string(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: "NOT_FOUND".to_string(),
            message: message.into(),
            kind: "error".to_string(),
        }
    }

    pub fn invalid_resource(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_RESOURCE".to_string(),
            message: message.into(),
            kind: "error".to_string(),
        }
    }
}

impl std::fmt::Display for KubernetesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for KubernetesError {}

impl From<ureq::Error> for KubernetesError {
    fn from(err: ureq::Error) -> Self {
        Self {
            code: "HTTP_ERROR".to_string(),
            message: err.to_string(),
            kind: "error".to_string(),
        }
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesConfig {
    pub api_server: Option<String>,
    pub token: Option<String>,
    pub namespace: Option<String>,
    pub ca_cert: Option<String>,
    pub timeout: Option<u64>,
}

impl KubernetesConfig {
    pub fn new() -> Self {
        Self {
            api_server: None,
            token: None,
            namespace: Some("default".to_string()),
            ca_cert: None,
            timeout: Some(30),
        }
    }

    pub fn with_server(mut self, server: impl Into<String>) -> Self {
        self.api_server = Some(server.into());
        self
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl Default for KubernetesConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Metadata Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_timestamp: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
}

// ============================================================================
// Version Info
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubeVersion {
    pub major: String,
    pub minor: String,
    pub git_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_tree_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub go_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub version: String,
    pub server_version: KubeVersion,
    pub kubernetes_version: KubeVersion,
}

// ============================================================================
// Node Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    #[serde(rename = "podCIDR", skip_serializing_if = "Option::is_none")]
    pub pod_cid_r: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unschedulable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCondition {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_transition_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAddress {
    #[serde(rename = "type")]
    pub type_: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSystemInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_runtime_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kubelet_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kube_proxy_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operating_system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocatable: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<NodeCondition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<NodeAddress>>,
    #[serde(rename = "nodeInfo", skip_serializing_if = "Option::is_none")]
    pub node_info: Option<NodeSystemInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: KubernetesMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<NodeSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NodeStatus>,
}

// ============================================================================
// Pod Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPort {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub container_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub name: String,
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<ContainerPort>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<EnvVar>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodSpec {
    pub containers: Vec<Container>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminated: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    pub name: String,
    pub ready: bool,
    pub restart_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ContainerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_statuses: Option<Vec<ContainerStatus>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_i_ps: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pod {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: KubernetesMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<PodSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PodStatus>,
}

// ============================================================================
// Deployment Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelSelector {
    #[serde(rename = "matchLabels", skip_serializing_if = "Option::is_none")]
    pub match_labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodTemplateSpec {
    pub metadata: KubernetesMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<PodSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,
    pub selector: LabelSelector,
    pub template: PodTemplateSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentCondition {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentStatus {
    pub observed_generation: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_replicas: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready_replicas: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_replicas: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<DeploymentCondition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: KubernetesMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<DeploymentSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DeploymentStatus>,
}

// ============================================================================
// Service Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePort {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSpec {
    pub ports: Vec<ServicePort>,
    pub selector: HashMap<String, String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerIngress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingress: Option<Vec<LoadBalancerIngress>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    #[serde(rename = "loadBalancer", skip_serializing_if = "Option::is_none")]
    pub load_balancer: Option<LoadBalancerStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: KubernetesMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<ServiceSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ServiceStatus>,
}

// ============================================================================
// ConfigMap Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMap {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: KubernetesMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, String>>,
}

// ============================================================================
// Secret Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: KubernetesMetadata,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, String>>,
}

// ============================================================================
// Generic Resource Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesResource {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: KubernetesMetadata,
    #[serde(flatten)]
    pub spec: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct WatchStream {
    pub kind: String,
    pub objects: Vec<serde_json::Value>,
}

// ============================================================================
// Main Trait Definition
// ============================================================================

#[async_trait]
pub trait KubernetesClient: Send + Sync {
    // Initialization & Connection
    async fn initialize(&mut self, config: KubernetesConfig) -> Result<()>;
    async fn test_connection(&self) -> Result<bool>;

    // Cluster Information
    async fn get_cluster_info(&self) -> Result<ClusterInfo>;
    async fn list_namespaces(&self) -> Result<Vec<String>>;
    async fn get_nodes(&self) -> Result<Vec<Node>>;

    // Pod Operations
    async fn get_pods(&self, namespace: &str) -> Result<Vec<Pod>>;
    async fn get_pod(&self, namespace: &str, name: &str) -> Result<Option<Pod>>;
    async fn create_pod(&self, namespace: &str, pod: &Pod) -> Result<Pod>;
    async fn update_pod(&self, namespace: &str, name: &str, pod: &Pod) -> Result<Pod>;
    async fn delete_pod(&self, namespace: &str, name: &str) -> Result<()>;
    async fn get_pod_logs(&self, namespace: &str, name: &str) -> Result<String>;
    async fn exec_in_pod(&self, namespace: &str, name: &str, command: &str) -> Result<String>;

    // Deployment Operations
    async fn get_deployments(&self, namespace: &str) -> Result<Vec<Deployment>>;
    async fn get_deployment(&self, namespace: &str, name: &str) -> Result<Option<Deployment>>;
    async fn create_deployment(
        &self,
        namespace: &str,
        deployment: &Deployment,
    ) -> Result<Deployment>;
    async fn update_deployment(
        &self,
        namespace: &str,
        name: &str,
        deployment: &Deployment,
    ) -> Result<Deployment>;
    async fn delete_deployment(&self, namespace: &str, name: &str) -> Result<()>;
    async fn scale_deployment(
        &self,
        namespace: &str,
        name: &str,
        replicas: i32,
    ) -> Result<Deployment>;

    // Service Operations
    async fn get_services(&self, namespace: &str) -> Result<Vec<Service>>;
    async fn get_service(&self, namespace: &str, name: &str) -> Result<Option<Service>>;
    async fn create_service(&self, namespace: &str, service: &Service) -> Result<Service>;
    async fn update_service(
        &self,
        namespace: &str,
        name: &str,
        service: &Service,
    ) -> Result<Service>;
    async fn delete_service(&self, namespace: &str, name: &str) -> Result<()>;

    // ConfigMap Operations
    async fn get_configmaps(&self, namespace: &str) -> Result<Vec<ConfigMap>>;
    async fn get_configmap(&self, namespace: &str, name: &str) -> Result<Option<ConfigMap>>;
    async fn create_configmap(&self, namespace: &str, configmap: &ConfigMap) -> Result<ConfigMap>;
    async fn update_configmap(
        &self,
        namespace: &str,
        name: &str,
        configmap: &ConfigMap,
    ) -> Result<ConfigMap>;
    async fn delete_configmap(&self, namespace: &str, name: &str) -> Result<()>;

    // Secret Operations
    async fn get_secrets(&self, namespace: &str) -> Result<Vec<Secret>>;
    async fn get_secret(&self, namespace: &str, name: &str) -> Result<Option<Secret>>;
    async fn create_secret(&self, namespace: &str, secret: &Secret) -> Result<Secret>;
    async fn update_secret(&self, namespace: &str, name: &str, secret: &Secret) -> Result<Secret>;
    async fn delete_secret(&self, namespace: &str, name: &str) -> Result<()>;

    // Generic Resource Operations
    async fn apply_resource(
        &self,
        namespace: &str,
        resource: &KubernetesResource,
    ) -> Result<KubernetesResource>;
    async fn delete_resource(&self, namespace: &str, kind: &str, name: &str) -> Result<()>;
    async fn watch_resources(&self, namespace: &str, kind: &str) -> Result<WatchStream>;
}

// ============================================================================
// Helper Functions
// ============================================================================

pub fn create_kubernetes_config() -> KubernetesConfig {
    KubernetesConfig::new()
}

pub fn create_pod_template(name: impl Into<String>, image: impl Into<String>) -> Pod {
    Pod {
        api_version: "v1".to_string(),
        kind: "Pod".to_string(),
        metadata: KubernetesMetadata {
            name: name.into(),
            namespace: Some("default".to_string()),
            labels: None,
            annotations: None,
            creation_timestamp: None,
            generation: None,
            resource_version: None,
            uid: None,
        },
        spec: Some(PodSpec {
            containers: vec![Container {
                name: "app".to_string(),
                image: image.into(),
                ports: None,
                env: None,
                resources: None,
            }],
            restart_policy: Some("Always".to_string()),
        }),
        status: None,
    }
}

pub fn create_deployment_template(
    name: impl Into<String>,
    image: impl Into<String>,
    replicas: i32,
) -> Deployment {
    let name_str = name.into();
    Deployment {
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        metadata: KubernetesMetadata {
            name: name_str.clone(),
            namespace: Some("default".to_string()),
            labels: None,
            annotations: None,
            creation_timestamp: None,
            generation: None,
            resource_version: None,
            uid: None,
        },
        spec: Some(DeploymentSpec {
            replicas: Some(replicas),
            selector: LabelSelector {
                match_labels: Some(HashMap::from([("app".to_string(), name_str.clone())])),
            },
            template: PodTemplateSpec {
                metadata: KubernetesMetadata {
                    name: name_str,
                    namespace: Some("default".to_string()),
                    labels: Some(HashMap::from([("app".to_string(), "app".to_string())])),
                    annotations: None,
                    creation_timestamp: None,
                    generation: None,
                    resource_version: None,
                    uid: None,
                },
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "app".to_string(),
                        image: image.into(),
                        ports: None,
                        env: None,
                        resources: None,
                    }],
                    restart_policy: Some("Always".to_string()),
                }),
            },
        }),
        status: None,
    }
}
