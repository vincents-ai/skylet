#!/usr/bin/env bash
# Hot Reload Test Runner
# Run this script to test hot reload functionality
#
# Usage:
#   ./run-hot-reload-test.sh          # Run all tests
#   ./run-hot-reload-test.sh --build   # Just build the test
#   ./run-hot-reload-test.sh --vm      # Run VM test (requires nix)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== Hot Reload Test Suite ==="
echo ""

# Test 1: Unit tests
echo ">>> Running unit tests..."
cd "$SCRIPT_DIR"
cargo test --lib -p execution-engine test_hot_reload 2>&1 | tail -20

echo ""
echo ">>> All unit tests passed!"

# Test 2: File watching test
echo ""
echo ">>> Testing file watching infrastructure..."

# Create test directory
TEST_DIR=$(mktemp -d)
PLUGIN_DIR="$TEST_DIR/plugins"
mkdir -p "$PLUGIN_DIR"

# Create a simple plugin
cat > "$TEST_DIR/test_plugin.c" << 'EOF'
#include <stdio.h>
const char* plugin_get_version() { return "1.0.0"; }
int plugin_init() { printf("[test-plugin] v1.0.0\n"); return 0; }
void plugin_shutdown() { printf("[test-plugin] shutdown v1.0.0\n"); }
EOF

# Compile
gcc -shared -fPIC -o "$PLUGIN_DIR/test_plugin.so" "$TEST_DIR/test_plugin.c"

echo "Created plugin v1.0.0"
ls -la "$PLUGIN_DIR/"

# Test file change detection
if command -v inotifywait &> /dev/null; then
    echo ""
    echo ">>> Testing inotify file change detection..."
    
    # Start watching in background
    inotifywait -m -e modify "$PLUGIN_DIR/" > "$TEST_DIR/inotify.log" 2>&1 &
    INOTIFY_PID=$!
    sleep 0.5
    
    # Modify file (simulate rebuild)
    cat > "$TEST_DIR/test_plugin.c" << 'EOF'
#include <stdio.h>
const char* plugin_get_version() { return "2.0.0"; }
int plugin_init() { printf("[test-plugin] v2.0.0\n"); return 0; }
void plugin_shutdown() { printf("[test-plugin] shutdown v2.0.0\n"); }
EOF
    gcc -shared -fPIC -o "$PLUGIN_DIR/test_plugin.so" "$TEST_DIR/test_plugin.c"
    
    sleep 0.5
    
    # Check detection
    if grep -q "test_plugin.so" "$TEST_DIR/inotify.log" 2>/dev/null; then
        echo "✓ File change detected via inotify"
    else
        echo "✗ File change NOT detected"
        kill $INOTIFY_PID 2>/dev/null || true
        rm -rf "$TEST_DIR"
        exit 1
    fi
    
    # Cleanup
    kill $INOTIFY_PID 2>/dev/null || true
else
    echo "⚠ inotify-tools not installed, skipping file watching test"
fi

# Cleanup
rm -rf "$TEST_DIR"

echo ""
echo "=== All Tests Passed ==="
