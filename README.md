# HBCN Constrainer

A Rust-based tool for timing constraint generation in Half-buffer Channel Networks (HBCNs). This tool is part of the **Pulsar** framework and is specifically designed to generate timing constraints for **asynchronous circuits synthesis using Cadence Genus**.

The tool analyses circuit graphs and produces timing constraints in various formats, enabling efficient synthesis and optimisation of asynchronous digital circuits in modern EDA flows.

## Installation

```bash
# Clone the repository
git clone <repository-url>
cd hbcn_constrainer

# Build the project
cargo build --release

# The binary will be available at target/release/hbcn
```

## Usage

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

## Dependencies

- **Rust**: Version 2024 edition
- **Gurobi**: Optimisation solver (for constraint generation)
- **Additional Crates**: 
  - `petgraph`: Graph data structures
  - `clap`: Command-line parsing
  - `anyhow`: Error handling
  - `vcd`: VCD file generation
  - `prettytable-rs`: Report formatting

## Architecture

### Core Components

1. **Structural Graph Parser**: Parses input circuit descriptions
2. **HBCN Converter**: Converts structural graphs to HBCN representation
3. **Constraint Algorithms**: 
   - Proportional constraint generation
   - Pseudoclock constraint generation
4. **Output Generators**: SDC, CSV, VCD, and report writers
5. **Analysis Engine**: Critical path and cycle analysis

### Algorithm Overview

The constrainer uses mathematical optimisation to generate timing constraints that:
- Ensure proper half-buffer channel network timing
- Minimise cycle time while meeting delay requirements
- Handle both forward and backward path constraints
- Support various optimisation objectives

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
**Test Suite**: ✅ 12/12 tests passing  
**Supported Platforms**: macOS, Linux  
**Dependencies**: Gurobi solver required for optimisation
