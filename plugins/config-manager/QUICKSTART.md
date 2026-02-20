# Config Manager - Quick Start Guide

Get up and running with the Config Manager plugin in minutes.

## Table of Contents

1. [Installation & Setup](#installation--setup)
2. [Basic Usage](#basic-usage)
3. [Usage Patterns](#usage-patterns)
4. [Integration Examples](#integration-examples)
5. [Troubleshooting](#troubleshooting)
6. [Performance Tips](#performance-tips)

---

## Installation & Setup

### Prerequisites

- Skylet plugin system installed
- Rust 1.70+ (if building from source)
- nix develop environment available

### Build Plugin

```bash
cd skylet

# Enter Nix development environment
nix develop

# Build the plugin
cargo build -p config-manager --release

# Verify binary
ls -lh target/release/libconfig_manager.so
```

### Create Configuration File

```bash
mkdir -p /etc/skylet

cat > /etc/skylet/config.toml << 'EOF'
[database]
path = "./data/skylet.db"
node_id = 1
data_dir = "./data"
EOF
```

Or as JSON:

```json
{
  "database": {
    "path": "./data/skylet.db",
    "node_id": 1,
    "data_dir": "./data"
  }
}
```

---

## Basic Usage

### Load Configuration from File

```bash
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d '{"path": "/etc/skylet/config.toml"}' | jq .
```

### Retrieve Current Configuration

```bash
curl -X GET http://localhost:8080/plugin/config-manager/config_get | jq '.data'

# Extract database section
curl -X GET http://localhost:8080/plugin/config-manager/config_get | \
  jq '.data.database'
```

### Update Configuration

```bash
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{
    "database": {
      "path": "./data/skylet.db",
      "node_id": 2,
      "data_dir": "./data"
    }
  }' | jq .
```

### Validate Configuration

```bash
curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .
```

### Export Configuration

```bash
# Export as JSON
curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' > config_backup.json

# Export as TOML
curl -X GET http://localhost:8080/plugin/config-manager/config_export_toml | \
  jq -r '.data' > config_export.toml

# Export as YAML
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq -r '.data' > config.yaml
```

---

## Usage Patterns

### Multi-Environment Configuration

```bash
#!/bin/bash
CONFIG_HOME="/etc/skylet"

load_env_config() {
    local env=$1
    local config_file="$CONFIG_HOME/config.$env.toml"

    echo "Loading $env configuration..."
    curl -X POST http://localhost:8080/plugin/config-manager/config_load \
      -H "Content-Type: application/json" \
      -d "{\"path\": \"$config_file\"}" | jq .
}

# Switch environments
load_env_config "dev"
load_env_config "staging"
load_env_config "production"
```

### Configuration Version Control

```bash
#!/bin/bash
CONFIG_REPO="/opt/config-repo"
timestamp=$(date +%Y%m%d_%H%M%S)

cd "$CONFIG_REPO"

curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' > "config_$timestamp.json"

git add "config_$timestamp.json"
git commit -m "config: snapshot $timestamp"
git tag "config-$timestamp"
```

### Configuration Synchronization Across Nodes

```bash
#!/bin/bash
NODES=("node1.example.com" "node2.example.com" "node3.example.com")

# Export from primary
curl -X GET http://localhost:8080/plugin/config-manager/config_export_toml | \
  jq -r '.data' > /tmp/config_sync.toml

# Sync to all nodes
for node in "${NODES[@]}"; do
    scp /tmp/config_sync.toml "skylet@$node:/tmp/config_sync.toml"
    ssh "skylet@$node" \
      "curl -X POST http://localhost:8080/plugin/config-manager/config_load \
        -H 'Content-Type: application/json' \
        -d '{\"path\": \"/tmp/config_sync.toml\"}'"
done
```

---

## Integration Examples

### Kubernetes ConfigMap

```bash
# Export as YAML
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq -r '.data' > /tmp/skylet-config.yaml

# Create ConfigMap
kubectl create configmap skylet-config \
  --from-file=/tmp/skylet-config.yaml \
  -n skylet-system

kubectl apply -f /tmp/configmap.yaml
```

### Docker Compose

```yaml
version: '3.8'
services:
  skylet-node:
    image: skylet:latest
    volumes:
      - ./config.json:/etc/skylet/config.json:ro
    environment:
      CONFIG_FILE: /etc/skylet/config.json
    ports:
      - "8080:8080"
```

---

## Troubleshooting

### Configuration File Not Found

```bash
# Check file exists and is readable
ls -la /etc/skylet/config.toml

# Create if missing
mkdir -p /etc/skylet
cat > /etc/skylet/config.toml << 'EOF'
[database]
path = "./data/skylet.db"
node_id = 1
data_dir = "./data"
EOF
```

### Configuration Validation Failed

```bash
# Check current configuration
curl -X GET http://localhost:8080/plugin/config-manager/config_get | jq '.data.database'

# Fix: database path must not be empty, node_id must be > 0
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{"database": {"path": "./data/skylet.db", "node_id": 1, "data_dir": "./data"}}'
```

### Invalid JSON Format

```bash
# Validate JSON before sending
echo '{"database": {"path": "./data/skylet.db"}}' | jq . || echo "Invalid JSON"
```

---

## Performance Tips

1. **Load once, update in-memory** - Avoid repeated file loads; use `config_set` for updates.
2. **Cache exports** - Export to a local file and reuse instead of re-exporting.
3. **Batch updates** - Combine multiple changes into a single `config_set` call.
4. **Use environment-specific files** - Keep separate `config.dev.toml`, `config.prod.toml`, etc.

---

## Next Steps

- See [Config Manager README](./README.md) for architecture details
- See [Config Manager API Reference](./API.md) for the full API
- See [Skylet Documentation](../../docs/) for system-wide patterns
