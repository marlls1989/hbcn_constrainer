#!/bin/bash

# HBCN Constraint Generation Benchmarks
# 
# This script provides convenient ways to run the constraint generation benchmarks

set -e

echo "🚀 HBCN Constraint Generation Benchmarks"
echo "========================================"
echo

# Check if graph files exist
echo "📂 Checking graph files..."
GRAPH_DIR="examples/structural_graphs"
if [ ! -d "$GRAPH_DIR" ]; then
    echo "❌ Error: Graph directory '$GRAPH_DIR' not found"
    echo "   Make sure .graph files are in the examples/structural_graphs/ directory"
    exit 1
fi

GRAPH_COUNT=$(find "$GRAPH_DIR" -name "*.graph" | wc -l)
echo "✅ Found $GRAPH_COUNT graph files in $GRAPH_DIR"
echo

# Function to run benchmarks with timing
run_benchmark() {
    local name="$1"
    local filter="$2"
    
    echo "🔬 Running $name benchmarks..."
    echo "   Filter: $filter"
    echo
    
    if [ -n "$filter" ]; then
        cargo bench "$filter"
    else
        cargo bench
    fi
    
    echo
    echo "✅ Completed $name benchmarks"
    echo "----------------------------------------"
    echo
}

# Parse command line arguments
case "${1:-all}" in
    "all")
        echo "🏁 Running all benchmarks..."
        run_benchmark "All" ""
        ;;
        
    "parsing")
        run_benchmark "Graph Parsing" "graph_parsing"
        ;;
        
    "pseudoclock")
        run_benchmark "Pseudoclock Constraints" "pseudoclock_constraints"
        ;;
        
    "proportional")
        run_benchmark "Proportional Constraints" "proportional_constraints"
        ;;
        
    "comparison")
        run_benchmark "Algorithm Comparison" "algorithm_comparison"
        ;;
        
    "sensitivity")
        run_benchmark "Parameter Sensitivity" "parameter_sensitivity"
        ;;
        
    "quick")
        echo "🏃‍♂️ Running quick benchmarks (parsing + one algorithm)..."
        run_benchmark "Graph Parsing" "graph_parsing"
        run_benchmark "Proportional Constraints" "proportional_constraints"
        ;;
        
    *)
        echo "Usage: $0 [all|parsing|pseudoclock|proportional|comparison|sensitivity|quick]"
        echo
        echo "Benchmark suites:"
        echo "  all         - Run all benchmarks (default)"
        echo "  parsing     - Graph parsing and HBCN conversion"
        echo "  pseudoclock - Pseudoclock constraint generation"
        echo "  proportional- Proportional constraint generation" 
        echo "  comparison  - Algorithm comparison (pseudoclock vs proportional)"
        echo "  sensitivity - Parameter sensitivity analysis"
        echo "  quick       - Quick test (parsing + proportional only)"
        echo
        echo "Graph-specific benchmarks:"
        echo "  You can also filter by graph name: cargo bench mac6"
        echo
        exit 1
        ;;
esac

echo "🎯 Benchmark Results:"
echo "   HTML reports: target/criterion/"
echo "   Terminal output above shows timing summaries"
echo
echo "📊 To view detailed HTML reports:"
echo "   open target/criterion/report/index.html"
echo
