# Config Manager - Quick Start Guide

Get up and running with the Config Manager plugin in minutes. This guide covers installation, basic usage, advanced patterns, real-world workflows, and integration scenarios.

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

- [ ] Skylet plugin system installed
- [ ] Rust 1.70+ (if building from source)
- [ ] Configuration files ready (TOML, JSON, or YAML format)
- [ ] Write permissions to configuration directories
- [ ] nix develop environment available
- [ ] Basic understanding of configuration management

### Step 1: Verify Prerequisites

```bash
#!/bin/bash
# Check Rust installation
rustc --version
cargo --version

# Verify Nix environment
nix --version
which nix develop

# Verify plugin directory exists
ls -la /home/shift/code/vincents-ai/skylet/plugins/config-manager/
```

### Step 2: Enter Development Environment

```bash
# Navigate to repository root
cd /home/shift/code/vincents-ai

# Enter Nix development environment
nix develop

# Verify environment variables
echo "IN_NIX_SHELL: $IN_NIX_SHELL"
which cargo
```

### Step 3: Build Plugin

```bash
# Navigate to plugin directory
cd /home/shift/code/vincents-ai/skylet/plugins/config-manager

# Build release binary
cargo build --release

# Verify binary created
ls -lh target/release/libconfig_manager.so

# Expected output: -rw-r--r-- 1 shift users 1.2M Feb  3 14:20 libconfig_manager.so
```

### Step 4: Create Configuration Files

```bash
#!/bin/bash
# Create configuration directory
mkdir -p /etc/skylet

# Create TOML configuration
cat > /etc/skylet/config.toml << 'EOF'
[database]
path = "./data/marketplace.db"
node_id = 1
raft_nodes = ["1 localhost:8100 localhost:8200"]
election_timeout_ms = 5000
secret_raft = "<YOUR_RAFT_SECRET_HERE>"
secret_api = "<YOUR_API_SECRET_HERE>"
data_dir = "./data"

[tor]
socks_port = 9050
control_port = 9051
hidden_service_port = 8080

[monero]
daemon_url = "http://localhost:18081"
wallet_path = "./data/wallet"
wallet_rpc_port = 18083
network = "testnet"
auto_refresh = true
refresh_interval = 30

[discovery]
enabled = true
cache_ttl = 300
announce_interval = 60

[agents]
enabled = true
max_concurrent = 10
timeout_seconds = 300

[escrow]
enabled = true
timeout_blocks = 100
min_confirmations = 6

[payments]
enabled = true
provider = "stripe"
api_version = "2023-10-16"
EOF

# Create JSON configuration as alternative
cat > /etc/skylet/config.json << 'EOF'
{
  "database": {
    "path": "./data/marketplace.db",
    "node_id": 1,
    "raft_nodes": ["1 localhost:8100 localhost:8200"],
    "election_timeout_ms": 5000,
    "secret_raft": "MarketplaceRaftSecret1337",
    "secret_api": "MarketplaceApiSecret1337",
    "data_dir": "./data"
  },
  "tor": {
    "socks_port": 9050,
    "control_port": 9051,
    "hidden_service_port": 8080
  },
  "monero": {
    "daemon_url": "http://localhost:18081",
    "wallet_path": "./data/wallet",
    "wallet_rpc_port": 18083,
    "network": "testnet",
    "auto_refresh": true,
    "refresh_interval": 30
  },
  "discovery": {
    "enabled": true,
    "cache_ttl": 300,
    "announce_interval": 60
  },
  "agents": {
    "enabled": true,
    "max_concurrent": 10,
    "timeout_seconds": 300
  },
  "escrow": {
    "enabled": true,
    "timeout_blocks": 100,
    "min_confirmations": 6
  },
  "payments": {
    "enabled": true,
    "provider": "stripe",
    "api_version": "2023-10-16"
  }
}
EOF

# Verify configuration files
echo "Configuration files created:"
ls -la /etc/skylet/config.*
```

### Step 5: Test Plugin Initialization

```bash
#!/bin/bash
# Create simple Rust test program
cat > test_plugin.rs << 'EOF'
use std::ffi::{CStr, CString};

#[repr(C)]
struct PluginContext {
    // Placeholder
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
enum PluginResult {
    Success = 0,
    Error = 1,
}

fn main() {
    unsafe {
        // Load plugin
        let lib = libloading::Library::new("./target/release/libconfig_manager.so")
            .expect("Failed to load plugin");

        // Get init function
        let init: libloading::Symbol<unsafe extern "C" fn(*const PluginContext) -> PluginResult> =
            lib.get(b"plugin_init").expect("Failed to get plugin_init");

        // Call init with null context
        let result = init(std::ptr::null());
        
        // Check result
        match result {
            PluginResult::Success => println!("Plugin initialized successfully"),
            PluginResult::Error => println!("Plugin initialization failed"),
        }

        // Get plugin info
        let get_info: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char> =
            lib.get(b"plugin_get_info").expect("Failed to get plugin_get_info");

        let info_ptr = get_info();
        let info_str = CStr::from_ptr(info_ptr).to_string_lossy();
        println!("Plugin info: {}", info_str);
    }
}
EOF

# Compile and run test
rustc test_plugin.rs --edition 2021 -L target/release/deps \
    --extern libloading=~/.cargo/registry/src/*/libloading-*/libloading.rlib 2>/dev/null || \
    echo "Test compilation requires proper Rust setup"
```

---

## Basic Usage

### Pattern 1: Load Configuration from File

Load configuration from a TOML, JSON, or YAML file:

```bash
#!/bin/bash
# Load configuration from TOML
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d '{"path": "/etc/skylet/config.toml"}' | jq .

# Response:
# {
#   "success": true,
#   "message": "Configuration loaded from /etc/skylet/config.toml"
# }
```

### Pattern 2: Retrieve Current Configuration

Get the entire current configuration:

```bash
#!/bin/bash
# Get all configuration
curl -X GET http://localhost:8080/plugin/config-manager/config_get | jq '.data'

# Extract specific section using jq
curl -X GET http://localhost:8080/plugin/config-manager/config_get | \
  jq '.data.database'

# Output:
# {
#   "path": "./data/marketplace.db",
#   "node_id": 1,
#   "raft_nodes": ["1 localhost:8100 localhost:8200"],
#   "election_timeout_ms": 5000,
#   ...
# }
```

### Pattern 3: Update Configuration Section

Update specific configuration sections without reloading from file:

```bash
#!/bin/bash
# Update just the Monero configuration
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{
    "monero": {
      "daemon_url": "http://monero.example.com:18081",
      "network": "mainnet",
      "auto_refresh": true,
      "refresh_interval": 60
    }
  }' | jq .

# Response:
# {
#   "success": true,
#   "message": "Configuration updated successfully"
# }
```

### Pattern 4: Validate Configuration

Verify configuration is valid before deployment:

```bash
#!/bin/bash
# Validate current configuration
curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .

# Response on success:
# {
#   "success": true,
#   "message": "Configuration is valid"
# }

# Response on validation error:
# {
#   "success": false,
#   "error": "Configuration validation failed: database.election_timeout_ms must be > 0"
# }
```

### Pattern 5: Export Configuration

Export configuration for backup or deployment to other systems:

```bash
#!/bin/bash
# Export as JSON
curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' > config_backup.json

# Export as TOML
curl -X GET http://localhost:8080/plugin/config-manager/config_export_toml | \
  jq -r '.data' > config_export.toml

# Export as YAML for Kubernetes
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq -r '.data' > config.yaml

# Verify exported file
file config_export.toml
head -20 config_export.toml
```

---

## Usage Patterns

### Pattern 1: Multi-Environment Configuration

Manage different configurations for development, staging, and production:

```bash
#!/bin/bash
# Configuration management script for multiple environments

CONFIG_HOME="/etc/skylet"
ENVIRONMENTS=("dev" "staging" "production")

# Function to load environment configuration
load_env_config() {
    local env=$1
    local config_file="$CONFIG_HOME/config.$env.toml"
    
    echo "Loading $env environment configuration..."
    
    curl -X POST http://localhost:8080/plugin/config-manager/config_load \
      -H "Content-Type: application/json" \
      -d "{\"path\": \"$config_file\"}" | jq .
}

# Function to export current configuration
export_current_config() {
    local env=$1
    local output_file="./config_$env.json"
    
    echo "Exporting $env configuration..."
    
    curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
      jq '.data' > "$output_file"
    
    echo "Exported to $output_file"
}

# Load development configuration
load_env_config "dev"

# Validate configuration
curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .

# Export for reference
export_current_config "dev"

# Switch to staging
load_env_config "staging"
curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .
export_current_config "staging"

# Switch to production
load_env_config "production"
curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .
export_current_config "production"
```

### Pattern 2: Environment-Specific Overrides

Load base configuration and override specific values per environment:

```bash
#!/bin/bash
# Base configuration loading with environment overrides

# Step 1: Load base configuration
echo "Step 1: Loading base configuration..."
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d '{"path": "/etc/skylet/config.base.toml"}' | jq .

# Step 2: Apply development overrides
echo "Step 2: Applying development overrides..."
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{
    "database": {
      "path": "./data/dev.db",
      "election_timeout_ms": 3000
    },
    "monero": {
      "network": "stagenet",
      "auto_refresh": true
    },
    "tor": {
      "socks_port": 9050
    }
  }' | jq .

# Step 3: Validate combined configuration
echo "Step 3: Validating configuration..."
curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .

# Step 4: Export final configuration
echo "Step 4: Exporting final configuration..."
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq -r '.data' > config.dev.yaml

echo "Final configuration exported to config.dev.yaml"
```

### Pattern 3: Configuration Version Control

Track configuration changes with Git integration:

```bash
#!/bin/bash
# Configuration versioning with Git

CONFIG_DIR="/etc/skylet/versions"
CONFIG_REPO="/opt/config-repo"

# Create configuration repository
mkdir -p "$CONFIG_REPO"
cd "$CONFIG_REPO"
git init
git config user.name "Config Manager"
git config user.email "config@example.com"

# Save current configuration with timestamp
timestamp=$(date +%Y%m%d_%H%M%S)

echo "Saving configuration version $timestamp..."

curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' > "config_$timestamp.json"

curl -X GET http://localhost:8080/plugin/config-manager/config_export_toml | \
  jq -r '.data' > "config_$timestamp.toml"

# Commit to Git
git add config_*
git commit -m "feat: Configuration snapshot $timestamp"

# Tag the commit
git tag "config-$timestamp"

# Show Git history
git log --oneline | head -10
git tag | tail -5
```

### Pattern 4: Configuration Synchronization Across Nodes

Synchronize configuration across multiple Skylet nodes:

```bash
#!/bin/bash
# Configuration synchronization across cluster

NODES=("node1.example.com" "node2.example.com" "node3.example.com")
CONFIG_FILE="/etc/skylet/config.production.toml"

echo "Starting configuration synchronization across cluster..."

# Step 1: Export configuration
echo "Step 1: Exporting configuration from primary..."
curl -X GET http://localhost:8080/plugin/config-manager/config_export_toml | \
  jq -r '.data' > /tmp/config_sync.toml

# Step 2: Verify exported configuration
echo "Step 2: Validating exported configuration..."
if [ ! -f /tmp/config_sync.toml ]; then
    echo "ERROR: Failed to export configuration"
    exit 1
fi

# Step 3: Sync to all cluster nodes
echo "Step 3: Syncing to cluster nodes..."
for node in "${NODES[@]}"; do
    echo "Syncing to $node..."
    
    scp /tmp/config_sync.toml "skylet@$node:/tmp/config_sync.toml"
    
    ssh "skylet@$node" << EOSSH
        # Load configuration on remote node
        curl -X POST http://localhost:8080/plugin/config-manager/config_load \
          -H "Content-Type: application/json" \
          -d '{"path": "/tmp/config_sync.toml"}'
        
        # Validate configuration
        curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .
        
        # Clean up temporary file
        rm /tmp/config_sync.toml
EOSSH
done

echo "Configuration synchronization complete"

# Step 4: Verify all nodes
echo "Step 4: Verifying configuration on all nodes..."
for node in "${NODES[@]}"; do
    echo "Checking $node..."
    ssh "skylet@$node" \
        "curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq '.success'"
done
```

### Pattern 5: Dynamic Configuration Reloading

Reload configuration without restarting services:

```bash
#!/bin/bash
# Dynamic configuration reloading with health checks

# Function to check service health
check_service_health() {
    local node=$1
    local max_retries=5
    local retry_count=0
    
    while [ $retry_count -lt $max_retries ]; do
        if curl -s -f "http://$node:8080/health" > /dev/null; then
            return 0
        fi
        retry_count=$((retry_count + 1))
        sleep 2
    done
    return 1
}

# Function to reload configuration
reload_configuration() {
    local node=$1
    local config_file=$2
    
    echo "Reloading configuration on $node..."
    
    # Load new configuration
    result=$(curl -X POST "http://$node:8080/plugin/config-manager/config_load" \
      -H "Content-Type: application/json" \
      -d "{\"path\": \"$config_file\"}")
    
    if echo "$result" | jq -e '.success' > /dev/null; then
        echo "Configuration loaded successfully"
        
        # Validate
        validation=$(curl -X GET "http://$node:8080/plugin/config-manager/config_validate")
        if echo "$validation" | jq -e '.success' > /dev/null; then
            echo "Configuration validated successfully"
            return 0
        else
            echo "Configuration validation failed"
            return 1
        fi
    else
        echo "Failed to load configuration"
        return 1
    fi
}

# Main reload process
NODES=("localhost:8080" "node2.example.com:8080")
NEW_CONFIG="/etc/skylet/config.updated.toml"

for node in "${NODES[@]}"; do
    echo "Processing node: $node"
    
    # Check health before reload
    if ! check_service_health "$node"; then
        echo "ERROR: Node $node is not healthy. Skipping reload."
        continue
    fi
    
    # Reload configuration
    if reload_configuration "$node" "$NEW_CONFIG"; then
        echo "Successfully reloaded configuration on $node"
    else
        echo "Failed to reload configuration on $node"
    fi
    
    # Verify health after reload
    if check_service_health "$node"; then
        echo "Node $node health verified after reload"
    else
        echo "WARNING: Node $node health check failed after reload"
    fi
done

echo "Configuration reload process complete"
```

---

## Real-World Workflows

### Workflow 1: Development to Production Promotion

Complete workflow for promoting configuration through environments:

```bash
#!/bin/bash
set -e

# Configuration promotion workflow
# Development -> Staging -> Production

ENVIRONMENTS=("development" "staging" "production")
CONFIG_REPO="/opt/config-management"
BACKUP_DIR="/opt/config-backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "=== Configuration Promotion Workflow ==="
echo "Timestamp: $TIMESTAMP"
echo ""

# Function to backup configuration
backup_config() {
    local env=$1
    local backup_path="$BACKUP_DIR/$env/$TIMESTAMP"
    
    mkdir -p "$backup_path"
    
    echo "Backing up $env configuration..."
    
    curl -X GET "http://$env.example.com:8080/plugin/config-manager/config_export_json" | \
      jq '.data' > "$backup_path/config.json"
    
    curl -X GET "http://$env.example.com:8080/plugin/config-manager/config_export_toml" | \
      jq -r '.data' > "$backup_path/config.toml"
    
    echo "Backed up to $backup_path"
}

# Function to load configuration
load_and_validate() {
    local env=$1
    local config_file=$2
    
    echo "Loading configuration on $env..."
    
    load_result=$(curl -s -X POST "http://$env.example.com:8080/plugin/config-manager/config_load" \
      -H "Content-Type: application/json" \
      -d "{\"path\": \"$config_file\"}")
    
    if ! echo "$load_result" | jq -e '.success' > /dev/null; then
        echo "ERROR: Failed to load configuration on $env"
        return 1
    fi
    
    echo "Configuration loaded, validating..."
    
    validate_result=$(curl -s -X GET "http://$env.example.com:8080/plugin/config-manager/config_validate")
    
    if ! echo "$validate_result" | jq -e '.success' > /dev/null; then
        echo "ERROR: Configuration validation failed on $env"
        echo "$validate_result" | jq '.error'
        return 1
    fi
    
    echo "Configuration validated on $env"
    return 0
}

# Step 1: Backup all environments
echo "Step 1: Backing up current configurations..."
for env in "${ENVIRONMENTS[@]}"; do
    backup_config "$env"
done

echo ""
echo "Step 2: Load configuration in development..."
DEV_CONFIG="/etc/skylet/config.development.toml"
if ! load_and_validate "development" "$DEV_CONFIG"; then
    echo "ERROR: Failed to load configuration in development"
    exit 1
fi

echo ""
echo "Step 3: Promote to staging..."
read -p "Continue with staging promotion? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    # Export from development and load to staging
    dev_config=$(curl -s -X GET "http://development.example.com:8080/plugin/config-manager/config_export_json" | jq '.data')
    
    # Apply with staging-specific overrides
    staging_config=$(echo "$dev_config" | jq '.database.election_timeout_ms = 6000 | .monero.network = "stagenet"')
    
    staging_result=$(curl -s -X POST "http://staging.example.com:8080/plugin/config-manager/config_set" \
      -H "Content-Type: application/json" \
      -d "$staging_config")
    
    if ! echo "$staging_result" | jq -e '.success' > /dev/null; then
        echo "ERROR: Failed to update staging configuration"
        echo "Rolling back..."
        # Restore from backup
        curl -s -X POST "http://staging.example.com:8080/plugin/config-manager/config_load" \
          -H "Content-Type: application/json" \
          -d "{\"path\": \"$BACKUP_DIR/staging/$TIMESTAMP/config.toml\"}"
        exit 1
    fi
    
    echo "Staging configuration updated successfully"
else
    echo "Staging promotion cancelled"
fi

echo ""
echo "Step 4: Promote to production..."
read -p "Continue with production promotion? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    # Get staging configuration
    staging_config=$(curl -s -X GET "http://staging.example.com:8080/plugin/config-manager/config_export_json" | jq '.data')
    
    # Apply production-specific overrides
    prod_config=$(echo "$staging_config" | jq '.monero.network = "mainnet" | .database.election_timeout_ms = 7000')
    
    prod_result=$(curl -s -X POST "http://production.example.com:8080/plugin/config-manager/config_set" \
      -H "Content-Type: application/json" \
      -d "$prod_config")
    
    if ! echo "$prod_result" | jq -e '.success' > /dev/null; then
        echo "ERROR: Failed to update production configuration"
        echo "Rolling back from backup..."
        curl -s -X POST "http://production.example.com:8080/plugin/config-manager/config_load" \
          -H "Content-Type: application/json" \
          -d "{\"path\": \"$BACKUP_DIR/production/$TIMESTAMP/config.toml\"}"
        exit 1
    fi
    
    echo "Production configuration updated successfully"
else
    echo "Production promotion cancelled"
fi

echo ""
echo "=== Configuration Promotion Complete ==="
echo "Backup location: $BACKUP_DIR"
echo "All configurations promoted and validated successfully"
```

### Workflow 2: Configuration-Driven Service Deployment

Deploy services based on configuration state:

```bash
#!/bin/bash
# Configuration-driven deployment workflow

CONFIG_FILE="/etc/skylet/config.deployment.toml"
DEPLOYMENT_TIMEOUT=600

echo "Starting configuration-driven deployment..."

# Step 1: Load configuration
echo "Step 1: Loading deployment configuration..."
load_result=$(curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d "{\"path\": \"$CONFIG_FILE\"}")

if ! echo "$load_result" | jq -e '.success' > /dev/null; then
    echo "ERROR: Failed to load configuration"
    exit 1
fi

# Step 2: Validate configuration
echo "Step 2: Validating configuration..."
validate_result=$(curl -X GET http://localhost:8080/plugin/config-manager/config_validate)

if ! echo "$validate_result" | jq -e '.success' > /dev/null; then
    echo "ERROR: Configuration validation failed"
    exit 1
fi

# Step 3: Extract deployment parameters
echo "Step 3: Extracting deployment parameters..."
config=$(curl -X GET http://localhost:8080/plugin/config-manager/config_get | jq '.data')

db_path=$(echo "$config" | jq -r '.database.path')
monero_network=$(echo "$config" | jq -r '.monero.network')
agents_enabled=$(echo "$config" | jq -r '.agents.enabled')
max_concurrent=$(echo "$config" | jq -r '.agents.max_concurrent')

echo "Database: $db_path"
echo "Monero Network: $monero_network"
echo "Agents Enabled: $agents_enabled"
echo "Max Concurrent: $max_concurrent"

# Step 4: Deploy based on configuration
echo "Step 4: Starting deployment based on configuration..."

# Example: Deploy database
mkdir -p "$(dirname "$db_path")"
echo "Created database directory: $(dirname "$db_path")"

# Example: Configure Monero wallet
case "$monero_network" in
    "mainnet")
        echo "Configuring Monero for mainnet..."
        # Configure mainnet wallet
        ;;
    "stagenet")
        echo "Configuring Monero for stagenet..."
        # Configure stagenet wallet
        ;;
    "testnet")
        echo "Configuring Monero for testnet..."
        # Configure testnet wallet
        ;;
esac

# Example: Configure agent pool
if [ "$agents_enabled" = "true" ]; then
    echo "Configuring agent pool with $max_concurrent concurrent workers..."
    # Configure agent pool based on max_concurrent
fi

echo "Deployment complete"
```

### Workflow 3: Configuration Drift Detection and Correction

Monitor and correct configuration drift across nodes:

```bash
#!/bin/bash
# Configuration drift detection and correction

NODES=("node1.example.com" "node2.example.com" "node3.example.com")
EXPECTED_CONFIG="/etc/skylet/config.authoritative.toml"
DRIFT_LOG="/var/log/config-drift.log"
CORRECTIONS_LOG="/var/log/config-corrections.log"

echo "Starting configuration drift detection..." | tee -a "$DRIFT_LOG"

# Function to get node configuration hash
get_config_hash() {
    local node=$1
    curl -s -X GET "http://$node:8080/plugin/config-manager/config_export_json" | \
      jq '.data' | sha256sum | cut -d' ' -f1
}

# Function to check configuration drift
check_drift() {
    local node=$1
    local expected_hash=$2
    local actual_hash=$(get_config_hash "$node")
    
    if [ "$actual_hash" != "$expected_hash" ]; then
        echo "DRIFT DETECTED on $node: expected=$expected_hash actual=$actual_hash" | tee -a "$DRIFT_LOG"
        return 1
    else
        echo "OK: $node configuration matches expected" | tee -a "$DRIFT_LOG"
        return 0
    fi
}

# Function to correct configuration drift
correct_drift() {
    local node=$1
    
    echo "Correcting configuration drift on $node..." | tee -a "$CORRECTIONS_LOG"
    
    result=$(curl -s -X POST "http://$node:8080/plugin/config-manager/config_load" \
      -H "Content-Type: application/json" \
      -d "{\"path\": \"$EXPECTED_CONFIG\"}")
    
    if echo "$result" | jq -e '.success' > /dev/null; then
        echo "Configuration correction successful on $node" | tee -a "$CORRECTIONS_LOG"
        return 0
    else
        echo "Configuration correction failed on $node" | tee -a "$CORRECTIONS_LOG"
        return 1
    fi
}

# Main drift detection loop
echo "Expected configuration hash: $(sha256sum "$EXPECTED_CONFIG" | cut -d' ' -f1)"
expected_hash=$(curl -s -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d "{\"path\": \"$EXPECTED_CONFIG\"}" && \
  curl -s -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' | sha256sum | cut -d' ' -f1)

for node in "${NODES[@]}"; do
    echo ""
    echo "Checking $node..."
    
    if ! check_drift "$node" "$expected_hash"; then
        read -p "Correct drift on $node? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            correct_drift "$node"
        fi
    fi
done

echo ""
echo "Configuration drift detection complete"
echo "Drift log: $DRIFT_LOG"
echo "Corrections log: $CORRECTIONS_LOG"
```

---

## Integration Examples

### Integration with Kubernetes Management Plugin

Deploy configurations as Kubernetes ConfigMaps:

```bash
#!/bin/bash
# Integration: Config Manager + Kubernetes Management Plugin

echo "Exporting configuration for Kubernetes deployment..."

# Export as YAML
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq -r '.data' > /tmp/skylet-config.yaml

# Create Kubernetes ConfigMap
kubectl create configmap skylet-config \
  --from-file=/tmp/skylet-config.yaml \
  -n skylet-system \
  --dry-run=client \
  -o yaml > /tmp/configmap.yaml

# Apply to cluster
kubectl apply -f /tmp/configmap.yaml

# Verify ConfigMap created
kubectl get configmap skylet-config -n skylet-system
kubectl describe configmap skylet-config -n skylet-system

# Mount in Deployment
cat > /tmp/deployment.yaml << 'EOF'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: skylet-node
  namespace: skylet-system
spec:
  template:
    spec:
      volumes:
      - name: config
        configMap:
          name: skylet-config
      containers:
      - name: skylet
        volumeMounts:
        - name: config
          mountPath: /etc/skylet/
          readOnly: true
EOF

kubectl apply -f /tmp/deployment.yaml
```

### Integration with Docker and Compose

Use Config Manager with Docker Compose:

```bash
#!/bin/bash
# Integration: Config Manager + Docker Compose

echo "Generating Docker Compose with managed configuration..."

# Export configuration as JSON
curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' > /tmp/config.json

# Create Docker Compose file
cat > docker-compose.yml << 'EOF'
version: '3.8'
services:
  skylet-node:
    image: skylet:latest
    volumes:
      - /tmp/config.json:/etc/skylet/config.json:ro
    environment:
      CONFIG_FILE: /etc/skylet/config.json
    ports:
      - "8080:8080"
    networks:
      - skylet
  
  monero-daemon:
    image: monero:latest
    environment:
      MONERO_NETWORK: testnet
    volumes:
      - monero-data:/data
    networks:
      - skylet

networks:
  skylet:
    driver: bridge

volumes:
  monero-data:
EOF

# Start services
docker-compose up -d

# Verify configuration loaded
docker-compose exec skylet-node curl -X GET http://localhost:8080/plugin/config-manager/config_get
```

### Integration with Git and CI/CD

Automated configuration management in CI/CD pipeline:

```bash
#!/bin/bash
# Integration: Config Manager + Git + CI/CD Pipeline

echo "Configuration management in CI/CD pipeline..."

# Step 1: Clone configuration repository
git clone https://github.com/myorg/skylet-config.git /tmp/config-repo
cd /tmp/config-repo

# Step 2: Load configuration based on branch
BRANCH=$(git rev-parse --abbrev-ref HEAD)
CONFIG_FILE="config.$BRANCH.toml"

if [ ! -f "$CONFIG_FILE" ]; then
    echo "ERROR: Configuration file not found: $CONFIG_FILE"
    exit 1
fi

echo "Loading configuration from $BRANCH branch: $CONFIG_FILE"

# Step 3: Load configuration
load_result=$(curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d "{\"path\": \"$(pwd)/$CONFIG_FILE\"}")

if ! echo "$load_result" | jq -e '.success' > /dev/null; then
    echo "ERROR: Failed to load configuration"
    exit 1
fi

# Step 4: Validate configuration
validate_result=$(curl -X GET http://localhost:8080/plugin/config-manager/config_validate)

if ! echo "$validate_result" | jq -e '.success' > /dev/null; then
    echo "ERROR: Configuration validation failed"
    exit 1
fi

# Step 5: Export for deployment
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq -r '.data' > /tmp/config.yaml

echo "Configuration ready for deployment"
echo "Config file: /tmp/config.yaml"
echo "Git commit: $(git rev-parse HEAD)"
echo "Git branch: $BRANCH"
```

---

## Troubleshooting

### Issue: Configuration File Not Found

**Error:**
```json
{
  "success": false,
  "error": "File not found: /etc/skylet/config.toml"
}
```

**Solution:**
```bash
# Check file exists
ls -la /etc/skylet/config.toml

# Check file permissions
stat /etc/skylet/config.toml

# Check path is correct
pwd
find /etc -name "config.toml" 2>/dev/null

# Create missing file
mkdir -p /etc/skylet
cat > /etc/skylet/config.toml << 'EOF'
[database]
path = "./data/marketplace.db"
node_id = 1
EOF
```

### Issue: Configuration Validation Failed

**Error:**
```json
{
  "success": false,
  "error": "Configuration validation failed: database.election_timeout_ms must be > 0"
}
```

**Solution:**
```bash
# Check current configuration
curl -X GET http://localhost:8080/plugin/config-manager/config_get | jq '.data.database'

# Update with valid values
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{"database": {"election_timeout_ms": 5000}}'

# Validate again
curl -X GET http://localhost:8080/plugin/config-manager/config_validate
```

### Issue: ConfigService Not Initialized

**Error:**
```json
{
  "success": false,
  "error": "ConfigService not initialized"
}
```

**Solution:**
```bash
# Ensure plugin is initialized
# Check plugin init logs
journalctl -u skylet-plugin-config-manager -f

# Manually reinitialize if needed
# Restart the plugin or service
systemctl restart skylet-plugin-config-manager

# Check status
systemctl status skylet-plugin-config-manager
```

### Issue: Invalid JSON Format

**Error:**
```json
{
  "success": false,
  "error": "Failed to parse config JSON: expected value at line 1 column 0"
}
```

**Solution:**
```bash
# Validate JSON before sending
echo '{"database": {"path": "./data.db"}}' | jq . || echo "Invalid JSON"

# Use jq to format properly
jq -c . < config.json

# Create valid JSON carefully
cat > valid_config.json << 'EOF'
{
  "database": {
    "path": "./data/marketplace.db"
  }
}
EOF

# Test with valid JSON
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d @valid_config.json
```

---

## Performance Tips

### Tip 1: Minimize Configuration Load Operations

Avoid frequent file loads; use in-memory updates instead:

```bash
#!/bin/bash
# GOOD: Load once, update multiple times
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d '{"path": "/etc/skylet/config.toml"}'

# Update specific fields instead of reloading
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{"monero": {"daemon_url": "http://new.url:18081"}}'

# BAD: Multiple load operations
# Don't repeatedly load from file
```

### Tip 2: Cache Configuration Exports

Export once and reuse instead of exporting repeatedly:

```bash
#!/bin/bash
# Export to file and reuse
curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' > /tmp/config_cache.json

# Read from cache multiple times
cat /tmp/config_cache.json | jq '.database'
cat /tmp/config_cache.json | jq '.monero'

# Invalidate cache when configuration changes
rm /tmp/config_cache.json
```

### Tip 3: Batch Configuration Updates

Combine multiple updates into single operation:

```bash
#!/bin/bash
# GOOD: Single batch update
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{
    "database": {"node_id": 2},
    "monero": {"network": "mainnet"},
    "tor": {"socks_port": 9050}
  }'

# BAD: Multiple sequential updates
# curl ... {"database": {"node_id": 2}}
# curl ... {"monero": {"network": "mainnet"}}
# curl ... {"tor": {"socks_port": 9050}}
```

### Tip 4: Use Environment-Specific Configuration Files

Avoid loading wrong configuration by using explicit files:

```bash
#!/bin/bash
# Create environment-specific files
ls -la /etc/skylet/config.*.toml

# Load specific file
ENV=${ENVIRONMENT:-production}
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d "{\"path\": \"/etc/skylet/config.$ENV.toml\"}"
```

### Tip 5: Implement Configuration Change Tracking

Log configuration changes for auditing:

```bash
#!/bin/bash
# Simple configuration change logging

CONFIG_LOG="/var/log/skylet-config-changes.log"

log_config_change() {
    local action=$1
    local description=$2
    local timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    echo "$timestamp | $action | $description" >> "$CONFIG_LOG"
}

# Log configuration load
log_config_change "LOAD" "Loaded from /etc/skylet/config.toml"

# Load configuration
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d '{"path": "/etc/skylet/config.toml"}'

# Log configuration update
log_config_change "UPDATE" "Updated monero.network to mainnet"

# Update configuration
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{"monero": {"network": "mainnet"}}'

# View change log
tail -20 "$CONFIG_LOG"
```

---

## Next Steps

1. **Deploy to Production:** Use the multi-environment workflow to promote your configuration through environments
2. **Integrate with Services:** Connect with other plugins like Kubernetes Management or GitOps Automation
3. **Implement Monitoring:** Set up configuration drift detection and automated correction
4. **Document Custom Configuration:** Create environment-specific configuration files for your deployment
5. **Automate with CI/CD:** Integrate configuration management into your CI/CD pipeline

For more information:
- See [Config Manager README](./README.md) for architecture and features
- See [Config Manager API Reference](./API.md) for detailed API documentation
- See [Skylet Documentation](../../../docs/) for system-wide integration patterns
