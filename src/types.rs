// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Common types used across the Skylet execution engine

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;

/// Network type for different network protocols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkType {
    Clearnet,
}

/// Health status for service monitoring
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Service endpoint representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceEndpoint {
    pub id: String,
    pub url: String,
    pub network_type: NetworkType,
    pub health_status: HealthStatus,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub endpoint_address: SocketAddr,
}

impl ServiceEndpoint {
    pub fn new(
        id: String,
        url: String,
        network_type: NetworkType,
        endpoint_address: SocketAddr,
    ) -> Self {
        Self {
            id,
            url,
            network_type,
            health_status: HealthStatus::Unknown,
            last_seen: chrono::Utc::now(),
            endpoint_address,
        }
    }

    pub fn is_expired(&self, ttl_seconds: i64) -> bool {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(self.last_seen);
        duration.num_seconds() > ttl_seconds
    }

    pub fn address(&self) -> &SocketAddr {
        &self.endpoint_address
    }
}

impl fmt::Display for NetworkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkType::Clearnet => write!(f, "clearnet")?,
        }
        Ok(())
    }
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy")?,
            HealthStatus::Degraded => write!(f, "degraded")?,
            HealthStatus::Unhealthy => write!(f, "unhealthy")?,
            HealthStatus::Unknown => write!(f, "unknown")?,
        }
        Ok(())
    }
}
