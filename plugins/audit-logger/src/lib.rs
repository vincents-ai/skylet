// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Audit Logger Plugin
//!
//! Logs security-relevant events to filesystem in JSON Lines format.
//! Supports daily rotation with 30-day retention.
//!
//! Log fields: timestamp, event_type, user, action, result, metadata

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};

static AUDIT_WRITER: Mutex<Option<AuditWriter>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: String,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub user: Option<String>,
    pub action: String,
    pub result: String,
    pub metadata: Option<serde_json::Value>,
}

struct AuditWriter {
    log_dir: PathBuf,
}

impl AuditWriter {
    fn new() -> Self {
        let log_dir = if cfg!(target_os = "linux") {
            PathBuf::from("/var/log/skylet/audit")
        } else {
            PathBuf::from("./data/audit")
        };

        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create audit log directory: {}", e);
        }

        Self { log_dir }
    }

    fn get_log_path(&self) -> PathBuf {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.log_dir.join(format!("audit-{}.jsonl", today))
    }

    fn write_event(&self, event: &AuditEvent) -> Result<(), String> {
        let log_path = self.get_log_path();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        let json = serde_json::to_string(event)
            .map_err(|e| format!("Failed to serialize event: {}", e))?;
        writeln!(file, "{}", json).map_err(|e| format!("Failed to write event: {}", e))?;

        self.cleanup_old_logs();

        Ok(())
    }

    fn cleanup_old_logs(&self) {
        let retention_days = 30;
        let cutoff = chrono::Local::now() - chrono::Duration::days(retention_days);

        if let Ok(entries) = fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        if let Ok(modified) = metadata.modified() {
                            let modified_datetime: chrono::DateTime<chrono::Local> =
                                modified.into();
                            if modified_datetime < cutoff {
                                let _ = fs::remove_file(entry.path());
                            }
                        }
                    }
                }
            }
        }
    }
}

fn log_event(
    event_type: &str,
    user: Option<&str>,
    action: &str,
    result: &str,
    metadata: Option<serde_json::Value>,
) {
    let event = AuditEvent {
        timestamp: chrono::Local::now().to_rfc3339(),
        event_type: event_type.to_string(),
        user: user.map(String::from),
        action: action.to_string(),
        result: result.to_string(),
        metadata,
    };

    if let Ok(mut guard) = AUDIT_WRITER.lock() {
        if let Some(writer) = guard.as_ref() {
            if let Err(e) = writer.write_event(&event) {
                eprintln!("Failed to write audit event: {}", e);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    if _context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    let writer = AuditWriter::new();

    if let Ok(mut guard) = AUDIT_WRITER.lock() {
        *guard = Some(writer);
    }

    log_event("plugin", None, "audit-logger-init", "success", None);

    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    log_event("plugin", None, "audit-logger-shutdown", "success", None);
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    std::ptr::null()
}
