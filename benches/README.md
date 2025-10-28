# HBCN Constraint Generation Benchmarks

This directory contains performance benchmarks for the HBCN constraint generation system.

## Overview

The benchmarks test various aspects of constraint generation performance:

- **Graph Parsing**: Measures the time to parse `.graph` files and convert them to HBCN format
- **Pseudoclock Constraints**: Benchmarks pseudoclock constraint generation algorithm
- **Proportional Constraints**: Benchmarks proportional constraint generation algorithm  
- **Algorithm Comparison**: Direct performance comparison between pseudoclock and proportional methods
- **Parameter Sensitivity**: Tests performance impact of different cycle times and margin values

## Graph Files

The benchmarks run against all structural graphs in `examples/structural_graphs/`:

- `ARV.graph` - ARV circuit graph
- `cyclic.graph` - Cyclic circuit example
- `mac4.graph` - MAC unit with 4 stages in inner loop
- `mac5.graph` - MAC unit with 5 stages in inner loop
- `mac6.graph` - MAC unit with 6 stages in inner loop
- `test.graph` - Test circuit

## Running Benchmarks

### All Benchmarks
```bash
cargo bench
```

### Specific Benchmark Groups
```bash
# Graph parsing only
cargo bench graph_parsing

# Constraint generation algorithms
cargo bench pseudoclock_constraints
cargo bench proportional_constraints

# Algorithm comparison
cargo bench algorithm_comparison

# Parameter sensitivity analysis
cargo bench parameter_sensitivity
```

### Benchmark with Specific Graph
```bash
# Filter by graph name (e.g., mac6)
cargo bench mac6
```

## Benchmark Parameters

Default benchmark parameters:
- **Cycle Time**: 10.0 ns
- **Minimal Delay**: 0.1 ns (100 ps)
- **Backward Margin**: 10% (0.9 factor)
- **Forward Margin**: 10% (0.9 factor)

Parameter sensitivity tests explore:
- Cycle times: 5.0, 10.0, 15.0, 20.0 ns
- Margins: 5%, 10%, 15%, 20%

## Output

Benchmarks generate:
- Terminal output with timing results
- HTML reports in `target/criterion/` (when `html_reports` feature is enabled)
- Statistical analysis including mean, std dev, and outlier detection

## Performance Metrics

The benchmarks measure:
- **Throughput**: Operations per second, scaled by graph complexity (nodes + edges)
- **Latency**: Absolute time per operation
- **Memory Usage**: Available through Criterion's profiling features
- **Scalability**: Performance trends across different graph sizes

## Understanding Results

- **Graph Parsing**: Should be dominated by I/O and parsing overhead
- **Constraint Generation**: Typically scales with graph complexity
- **Algorithm Comparison**: May show pseudoclock vs proportional trade-offs
- **Parameter Sensitivity**: Helps identify optimal constraint parameters

## Troubleshooting

If benchmarks fail to run:

1. **Missing Graph Files**: Ensure all `.graph` files exist in `examples/structural_graphs/`
2. **Solver Dependencies**: Check that LP solver (CBC or Gurobi) is properly configured
3. **Memory Issues**: Large graphs may require substantial memory for constraint generation
4. **Compilation Errors**: Run `cargo check --benches` to identify issues

## Extending Benchmarks

To add new benchmarks:

1. Add new graph files to `GRAPH_FILES` array
2. Create new benchmark functions following existing patterns
3. Add to `criterion_group!` macro
4. Document new benchmarks in this README
