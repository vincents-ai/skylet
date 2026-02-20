// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! OpenTelemetry Integration for RFC-0017
//!
//! This module provides the integration layer between Skylet tracing
//! and the OpenTelemetry SDK.

use std::sync::Arc;

use super::exporter::{ExportSpan, ExporterConfig, ExporterType, TracingExporter};
use super::span::SpanContext;
use super::TracingError;

/// Tracer configuration
#[derive(Debug, Clone)]
pub struct TracerConfig {
    /// Service name
    pub service_name: String,

    /// Sampler configuration
    pub sampler: SamplerConfig,

    /// Exporter configuration
    pub exporter: ExporterConfig,
}

impl Default for TracerConfig {
    fn default() -> Self {
        Self {
            service_name: "skylet".to_string(),
            sampler: SamplerConfig::AlwaysOn,
            exporter: ExporterConfig::disabled(),
        }
    }
}

/// Sampler configuration
#[derive(Debug, Clone)]
pub enum SamplerConfig {
    /// Always sample
    AlwaysOn,

    /// Never sample
    AlwaysOff,

    /// Sample with a fixed probability
    TraceIdRatioBased(f64),

    /// Parent-based sampling (follow parent decision)
    ParentBased(Box<SamplerConfig>),
}

/// OpenTelemetry tracer wrapper
pub struct OtelTracer {
    config: TracerConfig,
    exporter: Arc<dyn TracingExporter>,
}

impl std::fmt::Debug for OtelTracer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OtelTracer")
            .field("service_name", &self.config.service_name)
            .finish()
    }
}

impl OtelTracer {
    /// Create a new OpenTelemetry tracer
    pub fn new(config: TracerConfig) -> Result<Self, TracingError> {
        let exporter = create_exporter(&config.exporter)?;
        Ok(Self { config, exporter })
    }

    /// Check if a span should be sampled
    pub fn should_sample(&self, context: &SpanContext) -> bool {
        match &self.config.sampler {
            SamplerConfig::AlwaysOn => true,
            SamplerConfig::AlwaysOff => false,
            SamplerConfig::TraceIdRatioBased(ratio) => {
                // Simple hash-based sampling
                let hash = context
                    .trace_id
                    .as_str()
                    .chars()
                    .fold(0u64, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u64));
                (hash % 100) < (*ratio * 100.0) as u64
            }
            SamplerConfig::ParentBased(inner) => {
                // For now, delegate to inner sampler
                // In a full implementation, would check parent sampled flag first
                match **inner {
                    SamplerConfig::AlwaysOn => true,
                    SamplerConfig::AlwaysOff => false,
                    SamplerConfig::TraceIdRatioBased(ratio) => {
                        let hash = context
                            .trace_id
                            .as_str()
                            .chars()
                            .fold(0u64, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u64));
                        (hash % 100) < (ratio * 100.0) as u64
                    }
                    SamplerConfig::ParentBased(_) => true,
                }
            }
        }
    }

    /// Export a batch of spans
    pub fn export(&self, spans: &[ExportSpan]) -> Result<(), TracingError> {
        self.exporter.export(spans)
    }

    /// Flush pending exports
    pub fn flush(&self) -> Result<(), TracingError> {
        self.exporter.flush()
    }

    /// Shutdown the tracer
    pub fn shutdown(&self) -> Result<(), TracingError> {
        self.exporter.shutdown()
    }

    /// Get the service name
    pub fn service_name(&self) -> &str {
        &self.config.service_name
    }
}

/// Create an exporter based on configuration
fn create_exporter(config: &ExporterConfig) -> Result<Arc<dyn TracingExporter>, TracingError> {
    match config.exporter_type {
        ExporterType::None => Ok(Arc::new(super::exporter::NoOpExporter::new())),
        ExporterType::Otlp => {
            // For now, use in-memory exporter
            // Full implementation would create OTLP exporter
            Ok(Arc::new(super::exporter::InMemoryExporter::new()))
        }
        ExporterType::Jaeger => {
            // For now, use in-memory exporter
            // Full implementation would create Jaeger exporter
            Ok(Arc::new(super::exporter::InMemoryExporter::new()))
        }
        ExporterType::Custom(ref name) => Err(TracingError::ExporterError(format!(
            "Custom exporter '{}' not supported in this implementation",
            name
        ))),
    }
}

/// Initialize the global tracer
pub fn init_tracer(config: ExporterConfig) -> Result<(), TracingError> {
    let tracer_config = TracerConfig {
        service_name: config.service_name.clone(),
        sampler: SamplerConfig::TraceIdRatioBased(config.sample_rate),
        exporter: config,
    };

    let _tracer = OtelTracer::new(tracer_config)?;

    // In a full implementation, would set global tracer
    // opentelemetry::global::set_tracer_provider(...)

    Ok(())
}

/// Shutdown the global tracer
#[allow(dead_code)]
pub fn shutdown_tracer() -> Result<(), TracingError> {
    // In a full implementation, would shutdown global tracer
    // opentelemetry::global::shutdown_tracer_provider()
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracer_config() {
        let config = TracerConfig {
            service_name: "test-service".to_string(),
            sampler: SamplerConfig::AlwaysOn,
            exporter: ExporterConfig::disabled(),
        };

        let tracer = OtelTracer::new(config).unwrap();
        assert_eq!(tracer.service_name(), "test-service");
    }

    #[test]
    fn test_sampler_always_on() {
        let tracer = OtelTracer::new(TracerConfig {
            sampler: SamplerConfig::AlwaysOn,
            ..Default::default()
        })
        .unwrap();

        let ctx = SpanContext::root();
        assert!(tracer.should_sample(&ctx));
    }

    #[test]
    fn test_sampler_always_off() {
        let tracer = OtelTracer::new(TracerConfig {
            sampler: SamplerConfig::AlwaysOff,
            ..Default::default()
        })
        .unwrap();

        let ctx = SpanContext::root();
        assert!(!tracer.should_sample(&ctx));
    }

    #[test]
    fn test_sampler_ratio() {
        let tracer = OtelTracer::new(TracerConfig {
            sampler: SamplerConfig::TraceIdRatioBased(0.5),
            ..Default::default()
        })
        .unwrap();

        // Test multiple spans - should get roughly 50% sampled
        let mut sampled = 0;
        for _ in 0..100 {
            let ctx = SpanContext::root();
            if tracer.should_sample(&ctx) {
                sampled += 1;
            }
        }

        // Should be approximately 50, allow wide margin
        assert!(sampled > 20 && sampled < 80);
    }

    #[test]
    fn test_tracer_export() {
        let tracer = OtelTracer::new(TracerConfig {
            exporter: ExporterConfig::disabled(),
            ..Default::default()
        })
        .unwrap();

        let span = ExportSpan {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
            parent_span_id: None,
            name: "test_span".to_string(),
            start_time: 0,
            end_time: 1000,
            attributes: std::collections::HashMap::new(),
            events: vec![],
            status: crate::tracing_mod::exporter::ExportStatus::Ok,
            service_name: "test".to_string(),
        };

        assert!(tracer.export(&[span]).is_ok());
        assert!(tracer.flush().is_ok());
        assert!(tracer.shutdown().is_ok());
    }

    #[test]
    fn test_init_tracer() {
        let config = ExporterConfig::otlp("http://localhost:4317");
        assert!(init_tracer(config).is_ok());
        assert!(shutdown_tracer().is_ok());
    }
}
