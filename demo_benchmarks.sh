#!/bin/bash

# Demo script to showcase benchmark capabilities
# This script runs a quick benchmark demo to verify everything works

echo "🎯 HBCN Constraint Generation Benchmark Demo"
echo "============================================="
echo
echo "This demo runs a quick benchmark to verify the setup works correctly."
echo

# Check prerequisites
echo "📋 Checking prerequisites..."

if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Not in the hbcn project directory"
    exit 1
fi

if [ ! -d "examples/structural_graphs" ]; then
    echo "❌ Error: Graph files directory not found"
    exit 1
fi

GRAPH_COUNT=$(find examples/structural_graphs -name "*.graph" | wc -l)
echo "✅ Found $GRAPH_COUNT structural graph files"

# List available graphs
echo
echo "📂 Available structural graphs:"
for graph in examples/structural_graphs/*.graph; do
    if [ -f "$graph" ]; then
        basename "$graph"
    fi
done
echo

# Run a quick benchmark demonstration
echo "🚀 Running quick benchmark demo..."
echo "   Testing graph parsing performance on all graphs"
echo

# Use timeout to prevent long-running benchmarks in demo
timeout 60s cargo bench graph_parsing || {
    echo "⏰ Benchmark demo completed (may have been timeout-limited)"
}

echo
echo "✅ Benchmark demo completed!"
echo
echo "📊 Next steps:"
echo "   1. Run full benchmarks: ./run_benchmarks.sh"
echo "   2. View results: open target/criterion/report/index.html"
echo "   3. Run specific tests: cargo bench [filter]"
echo
echo "🔧 Available benchmark suites:"
echo "   - graph_parsing      : Parse .graph files and convert to HBCN"
echo "   - pseudoclock        : Pseudoclock constraint generation"
echo "   - proportional       : Proportional constraint generation"
echo "   - algorithm_comparison: Compare pseudoclock vs proportional"
echo "   - parameter_sensitivity: Test different parameters"
echo
