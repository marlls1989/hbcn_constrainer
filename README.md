# HBCN Constrainer

A Rust-based tool for timing constraint generation in Half-buffer Channel Networks (HBCNs). This tool is part of the **Pulsar** framework and is specifically designed to generate timing constraints for **asynchronous circuits synthesis using Cadence Genus**.

The tool analyses circuit graphs and produces timing constraints in various formats, enabling efficient synthesis and optimisation of asynchronous digital circuits in modern EDA flows.

## Installation

### Prerequisites

The HBCN Constrainer requires a Linear Programming (LP) solver for constraint generation. You can choose between two options:

#### Option 1: Coin CBC (Recommended - Default)
Coin CBC is an open-source LP solver that requires no additional installation:

```bash
# Clone the repository
git clone <repository-url>
cd hbcn_constrainer

# Build with Coin CBC (default)
cargo build --release

# The binary will be available at target/release/hbcn
```

#### Option 2: Gurobi (Commercial)
For maximum performance, you can use the commercial Gurobi solver:

1. **Install Gurobi**:
   - Download and install Gurobi from [gurobi.com](https://www.gurobi.com)
   - Set up your license (academic or commercial)
   - Ensure `gurobi_cl` is in your PATH

2. **Build with Gurobi**:
```bash
# Clone the repository
git clone <repository-url>
cd hbcn_constrainer

# Build with Gurobi
cargo build --release --features gurobi

# The binary will be available at target/release/hbcn
```

#### Option 3: Both Solvers
You can build with both solvers for maximum flexibility:

```bash
cargo build --release --features "gurobi coin_cbc"
```

## Usage

### LP Solver Selection

The HBCN Constrainer supports runtime solver selection through environment variables:

#### Runtime Solver Selection
```bash
# Use Gurobi solver (if available)
HBCN_LP_SOLVER=gurobi cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0

# Use Coin CBC solver
HBCN_LP_SOLVER=coin_cbc cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0

# Use default solver (Gurobi if available, otherwise Coin CBC)
cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0
```

#### Supported Solver Names
- `gurobi` - Gurobi commercial solver
- `coin_cbc`, `coin-cbc`, `cbc` - Coin CBC open-source solver

### Basic Constraint Generation

```bash
# Generate basic constraints
cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0

# With additional output formats
cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 \
    --csv constraints.csv --rpt analysis.rpt --vcd timing.vcd
```

### Algorithm Selection

```bash
# Use proportional constraints (default)
cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0

# Use pseudoclock constraints
cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --no-proportinal
```

### Margin Control

```bash
# Set forward and backward margins
cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 \
    --forward-margin 15 --backward-margin 20
```

### Advanced Options

```bash
# Disable forward completion optimisation
cargo run -- constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --no-forward-completion
```

## Command Line Options

- `input.graph`: Input structural graph file
- `--sdc <file>`: Output SDC constraints file (required)
- `-t, --cycle-time <ns>`: Target cycle time in nanoseconds (required)
- `-m, --minimal-delay <ns>`: Minimal propagation delay in nanoseconds (required)
- `--csv <file>`: Output CSV constraints file (optional)
- `--rpt <file>`: Output analysis report file (optional)
- `--vcd <file>`: Output VCD timing file (optional)
- `--no-proportinal`: Use pseudoclock constraints instead of proportional
- `--no-forward-completion`: Disable forward completion optimisation
- `--forward-margin <percent>`: Forward path margin percentage (0-99)
- `--backward-margin <percent>`: Backward path margin percentage (0-99)

## Input Format

The tool accepts structural graph files in the following format:

```
Port "input_port" [("connected_node", delay_value)]
DataReg "register_name" [("output_node", delay_value)]
Port "output_port" []
```

### Example Input
```
Port "clk" [("reg1", 5), ("reg2", 5)]
Port "data_in" [("reg1", 45)]
DataReg "reg1" [("logic", 30)]
DataReg "reg2" [("output", 40)]
Port "output" []
```

## Output Formats

### SDC (Synopsys Design Constraints)
Standard timing constraints format optimised for **Cadence Genus** synthesis of asynchronous circuits:
```tcl
create_clock -period 10.000 [get_port clk]
set_max_delay 5.500 -from [get_ports ...] -to [get_pins ...]
set_min_delay 1.000 -from [get_ports ...] -to [get_pins ...]
```

### CSV (Comma-Separated Values)
Tabular constraint data for analysis:
```csv
src,dst,cost,max_delay,min_delay
port_a,reg1,45,8.500,1.000
reg1,output,30,6.200,1.000
```

### VCD (Value Change Dump)
Timing visualisation data for waveform viewers.

### Report
Human-readable analysis including:
- Cycle time constraints
- Critical path analysis
- Slack calculations
- Token distribution

## Testing

Run the comprehensive test suite to verify installation:

```bash
# Test with default solver (Coin CBC)
cargo test

# Test with Gurobi (if available)
cargo test --features gurobi

# Test with both solvers
cargo test --features "gurobi coin_cbc"

# Test runtime solver selection
HBCN_LP_SOLVER=gurobi cargo test --features "gurobi coin_cbc"
HBCN_LP_SOLVER=coin_cbc cargo test --features "gurobi coin_cbc"
```

## Troubleshooting

### LP Solver Issues

#### "No LP solver backend available"
**Cause**: No LP solver features enabled during compilation.  
**Solution**: Build with at least one solver feature:
```bash
cargo build --features coin_cbc  # or --features gurobi
```

#### "Gurobi solver requested but gurobi feature not enabled"
**Cause**: `HBCN_LP_SOLVER=gurobi` but Gurobi feature not compiled in.  
**Solution**: Rebuild with Gurobi feature:
```bash
cargo build --features gurobi
```

#### "Invalid solver 'X' in HBCN_LP_SOLVER"
**Cause**: Unrecognized solver name in environment variable.  
**Solution**: Use valid solver names: `gurobi`, `coin_cbc`, `coin-cbc`, or `cbc`

#### Gurobi License Issues
**Cause**: Gurobi not properly installed or licensed.  
**Solution**: 
1. Install Gurobi from [gurobi.com](https://www.gurobi.com)
2. Set up license: `grbgetkey <license-key>`
3. Verify: `gurobi_cl --version`

## Dependencies

### Core Dependencies
- **Rust**: Version 2024 edition
- **LP Solver**: One of the following (see Installation section):
  - **Coin CBC** (default): Open-source LP solver via `coin_cbc` crate
  - **Gurobi** (optional): Commercial LP solver via `gurobi` crate

### Rust Crates
- `petgraph`: Graph data structures and algorithms
- `clap`: Command-line argument parsing
- `anyhow`: Error handling and propagation
- `vcd`: VCD (Value Change Dump) file generation
- `prettytable-rs`: Formatted table output for reports
- `regex`: Regular expression processing
- `string_cache`: String interning for performance
- `lazy_static`: Lazy static initialization
- `itertools`: Iterator utilities
- `rayon`: Parallel processing
- `ordered-float`: Floating-point ordering
- `lalrpop`: Parser generator for grammar files

### LP Solver Details

#### Coin CBC (Default)
- **Type**: Open-source Mixed Integer Linear Programming (MILP) solver
- **License**: Eclipse Public License 2.0
- **Performance**: Good for most constraint generation tasks
- **Installation**: Automatic via Cargo (no external dependencies)
- **Use Case**: Default choice for open-source deployments

#### Gurobi (Optional)
- **Type**: Commercial optimization solver
- **License**: Commercial (academic licenses available)
- **Performance**: Excellent for large-scale problems
- **Installation**: Requires separate Gurobi installation and license
- **Use Case**: High-performance constraint generation for large circuits

### System Requirements
- **Operating System**: macOS, Linux (Windows support via WSL)
- **Memory**: 4GB+ RAM recommended for large circuits
- **Disk Space**: ~100MB for binary and dependencies

## Architecture

### Core Components

1. **Structural Graph Parser**: Parses input circuit descriptions
2. **HBCN Converter**: Converts structural graphs to HBCN representation
3. **LP Solver Abstraction**: Unified interface for multiple LP solvers
   - Runtime solver selection via environment variables
   - Support for Coin CBC (open-source) and Gurobi (commercial)
   - Model-agnostic constraint generation
4. **Constraint Algorithms**: 
   - Proportional constraint generation
   - Pseudoclock constraint generation
5. **Output Generators**: SDC, CSV, VCD, and report writers
6. **Analysis Engine**: Critical path and cycle analysis

### Algorithm Overview

The constrainer uses mathematical optimisation to generate timing constraints that:
- Ensure proper half-buffer channel network timing
- Minimise cycle time while meeting delay requirements
- Handle both forward and backward path constraints
- Support various optimisation objectives

#### LP Solver Integration
The constraint generation process leverages Linear Programming (LP) solvers to:
- **Formulate timing constraints** as linear optimization problems
- **Solve for optimal cycle times** while respecting delay bounds
- **Generate constraint coefficients** for SDC output
- **Support multiple solver backends** for different deployment scenarios

The LP solver abstraction allows seamless switching between:
- **Coin CBC**: Open-source solver for general use
- **Gurobi**: Commercial solver for high-performance scenarios

## Pulsar Integration

This tool is a core component of the **Pulsar** asynchronous circuit synthesis framework. It serves as the timing constraint generation engine that:

- **Analyses HBCN circuits** generated from high-level descriptions
- **Produces synthesis-ready constraints** specifically formatted for Cadence Genus
- **Integrates seamlessly** with Pulsar's design flow for asynchronous circuits
- **Supports multiple constraint strategies** to optimise different design objectives

### Typical Pulsar Workflow

1. **Circuit Description** → Pulsar frontend generates HBCN graph
2. **HBCN Constrainer** → Generates timing constraints (this tool)
3. **Cadence Genus** → Synthesises asynchronous circuit with constraints
4. **Backend Tools** → Place & route with timing-aware optimisation

The generated SDC constraints ensure that Genus can properly synthesise asynchronous circuits while maintaining the required timing relationships for correct handshaking protocols.

## Contributing

1. Run the regression test suite before submitting changes
2. Add new tests for new functionality
3. Follow Rust coding conventions
4. Update documentation for API changes

## License

See [LICENSE](LICENSE) file for details.

## Status

**Current Version**: 0.1.2  
**Test Suite**: ✅ 35/35 integration tests passing  
**Supported Platforms**: macOS, Linux  
**LP Solvers**: Coin CBC (default), Gurobi (optional)  
**Runtime Selection**: Environment variable `HBCN_LP_SOLVER` supported
