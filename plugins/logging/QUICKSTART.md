# Logging Plugin - Quick Start Guide

Get up and running with the Logging Plugin in minutes. This guide covers installation, basic usage, advanced patterns, real-world workflows, and integration scenarios.

## Table of Contents

1. [Installation & Setup](#installation--setup)
2. [Basic Usage](#basic-usage)
3. [Usage Patterns](#usage-patterns)
4. [Real-World Workflows](#real-world-workflows)
5. [Integration Examples](#integration-examples)
6. [Troubleshooting](#troubleshooting)
7. [Performance Tips](#performance-tips)

---

## Installation & Setup

### Prerequisites Checklist

Before starting, verify you have:

- [ ] Skylet plugin system installed and running
- [ ] Rust 1.70+ (if building from source)
- [ ] nix develop environment available
- [ ] curl or similar HTTP client for testing
- [ ] Basic understanding of log levels and structured logging
- [ ] jq tool for JSON parsing (optional but helpful)

### Step 1: Verify Prerequisites

```bash
#!/bin/bash
# Check Rust installation
rustc --version
cargo --version

# Verify Nix environment
nix --version

# Check if curl is available
curl --version

# Check for jq (optional but helpful)
jq --version 2>/dev/null || echo "jq not installed"
```

### Step 2: Enter Development Environment

```bash
# Navigate to repository root
cd /home/shift/code/vincents-ai

# Enter Nix development environment
nix develop

# Verify environment
echo "IN_NIX_SHELL: $IN_NIX_SHELL"
which cargo
```

### Step 3: Build Plugin

```bash
# Navigate to plugin directory
cd /home/shift/code/vincents-ai/skylet/plugins/logging

# Build release binary
cargo build --release

# Verify binary created
ls -lh target/release/liblogging.so

# Expected output: -rw-r--r-- 1 shift users 1.1M Feb  3 14:20 liblogging.so
```

### Step 4: Load Plugin

```bash
#!/bin/bash
# Start Skylet with logging plugin

# Set up environment
export SKYLET_PLUGINS_DIR="/home/shift/code/vincents-ai/skylet/plugins"
export LOG_LEVEL="INFO"

# Start plugin system with logging plugin loaded
# This depends on your Skylet configuration
# Example:
systemctl start skylet-plugin-logging

# Or manually load if using plugin loader
skylet-loader --plugin-dir=$SKYLET_PLUGINS_DIR load-plugin logging

# Verify plugin is loaded
curl -X GET http://localhost:8080/plugin/logging/log/level/get
```

### Step 5: Verify Installation

```bash
#!/bin/bash
# Test plugin health and basic operations

echo "Step 1: Check plugin health..."
curl -X GET http://localhost:8080/plugin/logging/health | jq .

echo -e "\nStep 2: Get initial log level..."
curl -X GET http://localhost:8080/plugin/logging/log/level/get | jq .

echo -e "\nStep 3: Retrieve log events..."
curl -X GET http://localhost:8080/plugin/logging/log/events | jq '.count'

echo -e "\nInstallation verified successfully!"
```

---

## Basic Usage

### Pattern 1: Check Current Log Level

Get the current log level configuration:

```bash
#!/bin/bash
# Get current log level
curl -X GET http://localhost:8080/plugin/logging/log/level/get | jq .

# Example output:
# {
#   "level": "INFO",
#   "status": "success"
# }
```

### Pattern 2: Change Log Level

Temporarily change the log level for debugging or production environments:

```bash
#!/bin/bash
# Set log level to DEBUG for verbose logging
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "DEBUG"}' | jq .

# Verify change
curl -X GET http://localhost:8080/plugin/logging/log/level/get | jq .

# Set back to INFO for production
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "INFO"}'
```

### Pattern 3: Retrieve Log Events

Get all buffered log events for analysis:

```bash
#!/bin/bash
# Get all log events
curl -X GET http://localhost:8080/plugin/logging/log/events | jq .

# Count total events
curl -X GET http://localhost:8080/plugin/logging/log/events | jq '.count'

# Get first event
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[0] | fromjson'

# Get last 5 events
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[-5:] | map(fromjson)'
```

### Pattern 4: Filter Events by Level

Find specific log levels for troubleshooting:

```bash
#!/bin/bash
# Get all ERROR level events
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[] | fromjson | select(.level == "ERROR")'

# Get all WARNING level events
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[] | fromjson | select(.level == "WARN")'

# Count events by level
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[] | fromjson | .level' | sort | uniq -c
```

### Pattern 5: Export Logs

Export logs for external analysis or archival:

```bash
#!/bin/bash
# Export all events to JSON file
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events | map(fromjson)' > logs_export.json

# Export as CSV (convert JSON to CSV)
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq -r '.events[] | fromjson | [.timestamp, .level, .message] | @csv' > logs_export.csv

# Export with formatting
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events | map(fromjson) | sort_by(.timestamp)' > logs_sorted.json
```

---

## Usage Patterns

### Pattern 1: Dynamic Log Level Management

Adjust log verbosity based on system state:

```bash
#!/bin/bash
# Dynamic log level adjustment script

CURRENT_LEVEL=$(curl -s -X GET http://localhost:8080/plugin/logging/log/level/get | jq -r '.level')
echo "Current log level: $CURRENT_LEVEL"

case "$1" in
    "debug")
        echo "Enabling DEBUG logging..."
        curl -X POST http://localhost:8080/plugin/logging/log/level/set \
          -H "Content-Type: application/json" \
          -d '{"level": "DEBUG"}' | jq .
        ;;
    "info")
        echo "Setting INFO logging level..."
        curl -X POST http://localhost:8080/plugin/logging/log/level/set \
          -H "Content-Type: application/json" \
          -d '{"level": "INFO"}' | jq .
        ;;
    "error")
        echo "Setting ERROR logging level..."
        curl -X POST http://localhost:8080/plugin/logging/log/level/set \
          -H "Content-Type: application/json" \
          -d '{"level": "ERROR"}' | jq .
        ;;
    *)
        echo "Usage: $0 {debug|info|error}"
        exit 1
        ;;
esac

# Verify new level
curl -X GET http://localhost:8080/plugin/logging/log/level/get | jq '.level'
```

### Pattern 2: Log Analysis and Reporting

Generate reports from collected logs:

```bash
#!/bin/bash
# Log analysis and reporting script

OUTPUT_DIR="/tmp/log_reports"
mkdir -p "$OUTPUT_DIR"

echo "Generating log analysis report..."

# Export raw events
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events | map(fromjson)' > "$OUTPUT_DIR/raw_events.json"

# Summary statistics
curl -X GET http://localhost:8080/plugin/logging/log/events | jq '{
  total_events: .count,
  log_levels: (.events | map(fromjson) | group_by(.level) | map({level: .[0].level, count: length})),
  plugins: (.events | map(fromjson) | group_by(.plugin_name) | map({plugin: .[0].plugin_name, count: length})),
  time_range: {
    earliest: (.events | map(fromjson) | min_by(.timestamp).timestamp),
    latest: (.events | map(fromjson) | max_by(.timestamp).timestamp)
  }
}' > "$OUTPUT_DIR/summary.json"

# Error analysis
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[] | fromjson | select(.level == "ERROR")' > "$OUTPUT_DIR/errors.json"

echo "Report generated in $OUTPUT_DIR"
echo ""
echo "=== SUMMARY ==="
jq . "$OUTPUT_DIR/summary.json"
```

### Pattern 3: Continuous Log Monitoring

Monitor logs in real-time with periodic polling:

```bash
#!/bin/bash
# Continuous log monitoring script

INTERVAL=5  # Check every 5 seconds
LAST_COUNT=0

echo "Starting log monitoring (interval: ${INTERVAL}s)..."
echo "Press Ctrl+C to stop"

while true; do
    CURRENT_COUNT=$(curl -s -X GET http://localhost:8080/plugin/logging/log/events | jq '.count')
    
    if [ "$CURRENT_COUNT" -gt "$LAST_COUNT" ]; then
        NEW_EVENTS=$((CURRENT_COUNT - LAST_COUNT))
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] New events: $NEW_EVENTS (total: $CURRENT_COUNT)"
        
        # Show new events
        curl -s -X GET http://localhost:8080/plugin/logging/log/events | \
          jq '.events[-'"$NEW_EVENTS"':] | map(fromjson) | .[] | "\(.timestamp) [\(.level)] \(.message)"' | tr -d '"'
    fi
    
    LAST_COUNT=$CURRENT_COUNT
    sleep "$INTERVAL"
done
```

### Pattern 4: Log-Based Alerting

Trigger alerts based on log events:

```bash
#!/bin/bash
# Log-based alerting script

ALERT_LOG="/var/log/logging_alerts.log"
SLACK_WEBHOOK="https://hooks.slack.com/services/YOUR/WEBHOOK/URL"

echo "Monitoring for critical events..."

# Get recent events
curl -s -X GET http://localhost:8080/plugin/logging/log/events | jq '.events[]' | while read -r event_str; do
    event=$(echo "$event_str" | jq -s '.[] | fromjson')
    
    level=$(echo "$event" | jq -r '.level')
    message=$(echo "$event" | jq -r '.message')
    timestamp=$(echo "$event" | jq -r '.timestamp')
    
    # Alert on ERROR level
    if [ "$level" = "ERROR" ]; then
        alert_msg="ERROR ALERT at $timestamp: $message"
        echo "$alert_msg" >> "$ALERT_LOG"
        
        # Send to Slack if webhook configured
        if [ ! -z "$SLACK_WEBHOOK" ] && [ "$SLACK_WEBHOOK" != "https://hooks.slack.com/services/YOUR/WEBHOOK/URL" ]; then
            curl -X POST "$SLACK_WEBHOOK" \
              -H 'Content-Type: application/json' \
              -d "{\"text\": \"$alert_msg\"}" 2>/dev/null
        fi
    fi
done

# Show alerts
echo ""
echo "=== Recent Alerts ==="
tail -10 "$ALERT_LOG"
```

### Pattern 5: Trace ID Correlation

Correlate logs across distributed systems using trace IDs:

```bash
#!/bin/bash
# Trace ID correlation for distributed tracing

TRACE_ID="$1"

if [ -z "$TRACE_ID" ]; then
    echo "Usage: $0 <trace-id>"
    echo ""
    echo "Finding all logs for a specific trace ID..."
    
    # Show unique trace IDs
    curl -s -X GET http://localhost:8080/plugin/logging/log/events | \
      jq '.events[] | fromjson | .trace_id' | sort | uniq
    exit 1
fi

echo "Retrieving logs for trace ID: $TRACE_ID"
echo ""

# Get all events for trace ID, ordered by timestamp
curl -s -X GET http://localhost:8080/plugin/logging/log/events | \
  jq --arg tid "$TRACE_ID" '.events[] | fromjson | select(.trace_id == $tid) | "\(.timestamp) [\(.level)] [\(.span_id)] \(.message)"' | \
  sort | tr -d '"'

echo ""
echo "Total events for trace: $(curl -s -X GET http://localhost:8080/plugin/logging/log/events | jq --arg tid "$TRACE_ID" '[.events[] | fromjson | select(.trace_id == $tid)] | length')"
```

---

## Real-World Workflows

### Workflow 1: Debugging Production Issues

Complete workflow for investigating production problems:

```bash
#!/bin/bash
set -e

echo "=== Production Issue Investigation Workflow ==="
ISSUE_ID="$1"
INVESTIGATION_DIR="/tmp/investigations/$ISSUE_ID"

if [ -z "$ISSUE_ID" ]; then
    ISSUE_ID="$(date +%s)"
fi

mkdir -p "$INVESTIGATION_DIR"

echo "Investigation ID: $ISSUE_ID"
echo "Output directory: $INVESTIGATION_DIR"
echo ""

# Step 1: Collect current logs
echo "Step 1: Collecting current logs..."
curl -s -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events | map(fromjson) | sort_by(.timestamp)' > "$INVESTIGATION_DIR/all_events.json"

echo "Collected $(jq 'length' "$INVESTIGATION_DIR/all_events.json") events"

# Step 2: Extract errors
echo ""
echo "Step 2: Extracting error events..."
jq '[.[] | select(.level == "ERROR")]' "$INVESTIGATION_DIR/all_events.json" > "$INVESTIGATION_DIR/errors.json"

ERROR_COUNT=$(jq 'length' "$INVESTIGATION_DIR/errors.json")
echo "Found $ERROR_COUNT error events"

if [ "$ERROR_COUNT" -gt 0 ]; then
    echo ""
    echo "Error summary:"
    jq '.[] | "\(.timestamp): \(.message)"' "$INVESTIGATION_DIR/errors.json" | tr -d '"'
fi

# Step 3: Analyze by plugin
echo ""
echo "Step 3: Analyzing events by plugin..."
jq 'group_by(.plugin_name) | map({plugin: .[0].plugin_name, count: length, levels: (map(.level) | group_by(.) | map({level: .[0], count: length}))})' \
  "$INVESTIGATION_DIR/all_events.json" > "$INVESTIGATION_DIR/by_plugin.json"

echo "Plugins involved:"
jq '.[] | "\(.plugin): \(.count) events"' "$INVESTIGATION_DIR/by_plugin.json" | tr -d '"'

# Step 4: Time-based analysis
echo ""
echo "Step 4: Analyzing event timeline..."
jq 'group_by(.timestamp | .[0:10]) | map({date: .[0].timestamp[0:10], count: length})' \
  "$INVESTIGATION_DIR/all_events.json" > "$INVESTIGATION_DIR/timeline.json"

# Step 5: Create summary report
echo ""
echo "Step 5: Creating summary report..."
cat > "$INVESTIGATION_DIR/REPORT.md" << 'EOF'
# Production Issue Investigation Report

## Issue Details
- Investigation ID: $ISSUE_ID
- Timestamp: $(date)

## Summary Statistics
- Total Events: $(jq 'length' all_events.json)
- Error Events: $(jq 'length' errors.json)

## Error Timeline
$(jq -r '.[] | "\(.timestamp): \(.message)"' errors.json)

## Events by Plugin
$(jq -r '.[] | "\(.plugin): \(.count) events"' by_plugin.json)

## Investigation Files
- all_events.json: All collected events
- errors.json: Error events only
- by_plugin.json: Events grouped by plugin
- timeline.json: Events by date

EOF

echo ""
echo "=== Investigation Complete ==="
echo "Report saved to: $INVESTIGATION_DIR/REPORT.md"
echo "All files saved to: $INVESTIGATION_DIR"

ls -lah "$INVESTIGATION_DIR"
```

### Workflow 2: Performance Profiling with Logs

Use logs to identify performance bottlenecks:

```bash
#!/bin/bash
# Performance profiling using log analysis

echo "=== Performance Profiling Workflow ==="

# Enable DEBUG logging for detailed information
echo "Enabling DEBUG logging..."
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "DEBUG"}'

# Run performance test
echo "Running performance test..."
echo "Starting at: $(date)"

# Simulate workload (adjust as needed)
for i in {1..100}; do
    # Your performance test operations here
    curl -s http://localhost:8080/plugin/health > /dev/null
    [ $((i % 10)) -eq 0 ] && echo "Progress: $i/100"
done

echo "Test completed at: $(date)"

# Collect and analyze performance logs
echo ""
echo "Analyzing performance metrics..."

curl -s -X GET http://localhost:8080/plugin/logging/log/events | jq '{
    total_events: .count,
    debug_events: ([.events[] | fromjson | select(.level == "DEBUG")] | length),
    info_events: ([.events[] | fromjson | select(.level == "INFO")] | length),
    time_span: {
        first: (.events | map(fromjson) | min_by(.timestamp).timestamp),
        last: (.events | map(fromjson) | max_by(.timestamp).timestamp)
    }
}'

# Reset to INFO logging
echo ""
echo "Resetting log level to INFO..."
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "INFO"}'
```

### Workflow 3: Compliance and Audit Logging

Maintain audit logs for compliance requirements:

```bash
#!/bin/bash
# Compliance and audit logging workflow

AUDIT_LOG_DIR="/var/log/skylet-audit"
mkdir -p "$AUDIT_LOG_DIR"

echo "=== Compliance Audit Logging ==="
echo "Audit log directory: $AUDIT_LOG_DIR"

# Daily audit log export
DATE=$(date +%Y%m%d)
AUDIT_FILE="$AUDIT_LOG_DIR/audit_$DATE.json"

echo "Exporting audit logs for $DATE..."

# Export all events with audit metadata
curl -s -X GET http://localhost:8080/plugin/logging/log/events | jq '{
    export_timestamp: (now | todate),
    export_date: "'$DATE'",
    total_events: .count,
    events: .events | map(fromjson)
}' > "$AUDIT_FILE"

echo "Exported to: $AUDIT_FILE"

# Verify integrity
echo ""
echo "Verifying log integrity..."
jq 'length' "$AUDIT_FILE" | xargs -I {} echo "Audit file contains {} events"

# Generate audit summary
SUMMARY_FILE="$AUDIT_LOG_DIR/summary_$DATE.txt"
cat > "$SUMMARY_FILE" << EOF
Audit Log Summary - $DATE
Generated: $(date)

Total Events: $(jq '.count' "$AUDIT_FILE")
Audit File: $AUDIT_FILE

Log Levels:
$(jq '.events[] | .level' "$AUDIT_FILE" | sort | uniq -c)

Critical Events (ERROR):
$(jq '.events[] | select(.level == "ERROR") | "\(.timestamp): \(.message)"' "$AUDIT_FILE" | tr -d '"')

Archive for compliance:
EOF

echo ""
echo "Audit summary:"
cat "$SUMMARY_FILE"

# Archive logs older than 30 days
echo ""
echo "Archiving old logs..."
find "$AUDIT_LOG_DIR" -name "audit_*.json" -mtime +30 -exec gzip {} \; -print
find "$AUDIT_LOG_DIR" -name "summary_*.txt" -mtime +30 -exec gzip {} \; -print
```

---

## Integration Examples

### Integration with Kubernetes

Use Kubernetes native logging with Logging Plugin:

```bash
#!/bin/bash
# Kubernetes integration

echo "=== Kubernetes Integration ==="

# Export logs as Kubernetes ConfigMap
curl -s -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events | map(fromjson)' > /tmp/logs.json

# Create ConfigMap
kubectl create configmap skylet-logs \
  --from-file=/tmp/logs.json \
  -n skylet-system \
  --dry-run=client \
  -o yaml | kubectl apply -f -

echo "Logs exported to Kubernetes ConfigMap"

# View in cluster
kubectl get configmap skylet-logs -n skylet-system
```

### Integration with ELK Stack

Send logs to Elasticsearch for analysis:

```bash
#!/bin/bash
# ELK Stack integration

ELASTIC_HOST="elasticsearch:9200"
ELASTIC_INDEX="skylet-logs"

echo "=== ELK Stack Integration ==="

# Get events and send to Elasticsearch
curl -s -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[] | fromjson' | while read -r event; do
    
    # Send each event to Elasticsearch
    curl -X POST "$ELASTIC_HOST/$ELASTIC_INDEX/_doc" \
      -H "Content-Type: application/json" \
      -d "$event"
done

echo "Events indexed in Elasticsearch"
```

### Integration with Monitoring Systems

Feed logs to Prometheus or similar monitoring:

```bash
#!/bin/bash
# Monitoring system integration

PROMETHEUS_PUSHGATEWAY="http://localhost:9091"

echo "=== Monitoring Integration ==="

# Calculate metrics from logs
curl -s -X GET http://localhost:8080/plugin/logging/log/events | jq '{
    events_total: .count,
    events_by_level: (.events | map(fromjson) | group_by(.level) | map({level: .[0].level, count: length}))
}' > /tmp/metrics.json

# Convert to Prometheus format
cat > /tmp/metrics.txt << 'EOF'
# HELP skylet_log_events_total Total number of logged events
# TYPE skylet_log_events_total gauge
EOF

jq '.events_by_level[] | "skylet_log_events{level=\"\(.level)\"} \(.count)"' /tmp/metrics.json | \
  tr -d '"' >> /tmp/metrics.txt

# Push to Prometheus Pushgateway
curl --data-binary @/tmp/metrics.txt "$PROMETHEUS_PUSHGATEWAY/metrics/job/skylet/instance/logger"

echo "Metrics pushed to Prometheus Pushgateway"
```

---

## Troubleshooting

### Issue: Plugin Not Responding

**Problem:** HTTP requests to logging plugin return 404 or timeout

**Solution:**
```bash
# Check plugin is loaded
curl -s http://localhost:8080/plugin/logging/log/level/get || echo "Plugin not responding"

# Check system logs
journalctl -u skylet-plugins -f

# Try restarting plugin
systemctl restart skylet-plugin-logging

# Verify health
curl -X GET http://localhost:8080/plugin/logging/health
```

### Issue: Cannot Change Log Level

**Problem:** Setting log level fails with "Logging service lock failed"

**Solution:**
```bash
# This indicates a deadlock or initialization issue
# Check plugin initialization
curl -X GET http://localhost:8080/plugin/logging/health

# If unhealthy, restart
systemctl restart skylet-plugin-logging

# Try again
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "INFO"}'
```

### Issue: Events Not Being Collected

**Problem:** Getting events returns empty array

**Solution:**
```bash
# Events are only in memory - check if plugin is generating logs
# Verify log level is set appropriately
curl -X GET http://localhost:8080/plugin/logging/log/level/get

# Generate some events by running operations
# Then check again
curl -X GET http://localhost:8080/plugin/logging/log/events | jq '.count'

# Events are cleared on shutdown, so buffer is empty after restart
```

---

## Performance Tips

### Tip 1: Manage Buffer Size

The logging plugin buffers up to 1000 events. Retrieve them regularly to prevent loss:

```bash
#!/bin/bash
# Periodic log retrieval to prevent buffer overflow

while true; do
    count=$(curl -s -X GET http://localhost:8080/plugin/logging/log/events | jq '.count')
    
    if [ "$count" -gt 800 ]; then
        echo "Buffer approaching capacity ($count/1000), exporting..."
        
        # Export events
        curl -s -X GET http://localhost:8080/plugin/logging/log/events | \
          jq '.events | map(fromjson)' >> /var/log/skylet-events-$(date +%Y%m%d).json
    fi
    
    sleep 300  # Check every 5 minutes
done
```

### Tip 2: Optimize Log Level for Environment

Use appropriate log levels for your environment:

```bash
#!/bin/bash
# Environment-specific log levels

ENV=${ENVIRONMENT:-production}

case "$ENV" in
    development)
        LEVEL="TRACE"
        echo "Development: Using TRACE level for maximum debugging"
        ;;
    staging)
        LEVEL="DEBUG"
        echo "Staging: Using DEBUG level"
        ;;
    production)
        LEVEL="INFO"
        echo "Production: Using INFO level"
        ;;
esac

curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d "{\"level\": \"$LEVEL\"}"
```

### Tip 3: Archive Old Logs

Regularly archive and compress old log exports:

```bash
#!/bin/bash
# Log archival script

LOG_DIR="/var/log/skylet"
ARCHIVE_DIR="$LOG_DIR/archive"
RETENTION_DAYS=30

mkdir -p "$ARCHIVE_DIR"

# Find and archive old logs
find "$LOG_DIR" -name "*.json" -mtime +$RETENTION_DAYS \
  -exec gzip {} \;
  -exec mv {}.gz "$ARCHIVE_DIR/" \;

echo "Archived logs older than $RETENTION_DAYS days"
```

---

## Next Steps

1. **Configure Log Collection:** Set up automated log collection based on your requirements
2. **Integrate with Monitoring:** Connect logging with your monitoring and alerting system
3. **Implement Archival:** Create log archival strategy for compliance and storage management
4. **Monitor Performance:** Track log buffer usage and optimize log levels
5. **Setup Alerts:** Implement alert rules based on specific log patterns

For more information:
- See [Logging Plugin README](./README.md) for architecture and features
- See [Logging Plugin API Reference](./API.md) for detailed API documentation
- See [Skylet Documentation](../../../docs/) for system-wide integration patterns
