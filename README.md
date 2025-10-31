# HBCN Constrainer

A Rust-based tool for timing constraint generation in Half-buffer Channel Networks (HBCNs). This tool is part of the **Pulsar** framework and is specifically designed to generate timing constraints for **asynchronous circuits synthesis using Cadence Genus**.

The tool analyses circuit graphs and produces timing constraints in various formats, enabling efficient synthesis and optimisation of asynchronous digital circuits in modern EDA flows.

## Installation

### System Requirements

- **Operating System**: macOS, Linux (Windows support via WSL)
- **Rust**: Version 1.85+ (2024 edition)
- **Memory**: 4GB+ RAM recommended for large circuits
- **Disk Space**: ~100MB for binary and dependencies
- **LP Solver**: One of the following (see options below):
  - **Coin CBC**: Requires CBC library installation (see installation instructions)
  - **Gurobi**: Requires Gurobi Optimizer installation and license

### Prerequisites

#### Install Rust
If you don't have Rust installed, install it using rustup:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart your shell or source the environment
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

### Installation Methods

#### Method 1: From Source (Recommended)

1. **Clone the repository**:
```bash
git clone https://github.com/marlls989/hbcn_constrainer.git
cd hbcn_constrainer
```

2. **Choose your LP solver option** (see LP Solver Options below)

3. **Build the application**:
```bash
# For Coin CBC (default, no additional setup required)
cargo build --release

# For Gurobi (requires Gurobi installation)
cargo build --release --features gurobi

# For both solvers
cargo build --release --features "gurobi coin_cbc"
```

4. **Install globally** (optional):
```bash
# Install to ~/.cargo/bin
cargo install --path .

# Or copy the binary to your PATH
sudo cp target/release/hbcn /usr/local/bin/
```

#### Method 2: Using Cargo Install (if published)

```bash
# Install from crates.io (when available)
cargo install hbcn_constrainer

# Install with specific features
cargo install hbcn_constrainer --features gurobi
```

### LP Solver Options

The HBCN Constrainer requires a Linear Programming (LP) solver. Choose one of the following:

#### Option 1: Coin CBC (Recommended - Default)
Coin CBC is an open-source LP solver that requires the CBC library to be installed on your system:

**Pros:**
- Open source (Eclipse Public License 2.0)
- Good performance for most use cases
- No commercial license required

**Installation:**

1. **Install CBC library on your system**:

   **macOS (using Homebrew):**
   ```bash
   brew install coin-or-tools/coinor/cbc
   ```

   **Ubuntu/Debian:**
   ```bash
   sudo apt update
   sudo apt install coinor-cbc
   ```

   **CentOS/RHEL/Fedora:**
   ```bash
   # For CentOS/RHEL 8+
   sudo dnf install coin-or-Cbc
   
   # For older versions
   sudo yum install coin-or-Cbc
   ```

   **Arch Linux:**
   ```bash
   sudo pacman -S coin-or-cbc
   ```

2. **Verify CBC installation**:
   ```bash
   cbc --version
   ```

3. **Build with Coin CBC**:
   ```bash
   cargo build --release
   ```

#### Option 2: Gurobi (Commercial)
For maximum performance, you can use the commercial Gurobi solver:

**Pros:**
- Excellent performance for large-scale problems
- Advanced optimization features
- Academic licenses available

**Installation Steps:**

1. **Download and install Gurobi**:
   - Visit [gurobi.com](https://www.gurobi.com)
   - Download Gurobi Optimizer for your platform
   - Follow the installation instructions for your OS

2. **Set up your license**:
   ```bash
   grbgetkey <your-license-key>
   ```

3. **Verify Gurobi installation**:
   ```bash
   gurobi_cl --version
   ```

4. **Build with Gurobi**:
   ```bash
   cargo build --release --features gurobi
   ```

#### Option 3: Both Solvers (Maximum Flexibility)
Build with both solvers for runtime selection:

```bash
cargo build --release --features "gurobi coin_cbc"
```

### Verification

After installation, verify that everything works correctly:

```bash
# Check if the binary was built successfully
./target/release/hbcn --help

# Test with a simple example
echo 'Port "a" [("b", 20)]
Port "b" []' > test.graph

# Test expand command
./target/release/hbcn expand test.graph --output test.hbcn

# Test analyse command (with structural graph)
./target/release/hbcn analyse test.graph --structural --depth

# Test constrain command (if you have a solver)
./target/release/hbcn constrain test.graph --sdc test.sdc -t 10.0 -m 1.0

# Clean up test files
rm test.graph test.sdc
```

### Troubleshooting Installation

#### Common Issues

**"No LP solver backend available"**
- **Cause**: No LP solver features enabled during compilation
- **Solution**: Build with at least one solver feature:
  ```bash
  cargo build --features coin_cbc  # or --features gurobi
  ```

**"Gurobi solver requested but gurobi feature not enabled"**
- **Cause**: `HBCN_LP_SOLVER=gurobi` but Gurobi feature not compiled in
- **Solution**: Rebuild with Gurobi feature:
  ```bash
  cargo build --features gurobi
  ```

**"Invalid solver 'X' in HBCN_LP_SOLVER"**
- **Cause**: Unrecognized solver name in environment variable
- **Solution**: Use valid solver names: `gurobi`, `coin_cbc`, `coin-cbc`, or `cbc`

**Gurobi License Issues**
- **Cause**: Gurobi not properly installed or licensed
- **Solution**: 
  1. Install Gurobi from [gurobi.com](https://www.gurobi.com)
  2. Set up license: `grbgetkey <license-key>`
  3. Verify: `gurobi_cl --version`

**Build Errors on macOS**
- **Cause**: Missing system dependencies
- **Solution**: Install Xcode command line tools:
  ```bash
  xcode-select --install
  ```

**Build Errors on Linux**
- **Cause**: Missing development tools
- **Solution**: Install build essentials:
  ```bash
  # Ubuntu/Debian
  sudo apt update
  sudo apt install build-essential pkg-config libssl-dev
  
  # CentOS/RHEL/Fedora
  sudo yum groupinstall "Development Tools"
  sudo yum install pkgconfig openssl-devel
  ```

**CBC Library Not Found**
- **Cause**: CBC library not installed or not found by pkg-config
- **Solution**: Install CBC library and ensure pkg-config can find it:
  ```bash
  # Ubuntu/Debian
  sudo apt install coinor-cbc pkg-config
  
  # macOS
  brew install coin-or-tools/coinor/cbc pkg-config
  
  # Verify pkg-config can find CBC
  pkg-config --cflags --libs cbc
  ```

**CBC Compilation Errors**
- **Cause**: CBC library headers or libraries not properly installed
- **Solution**: Reinstall CBC with development headers:
  ```bash
  # Ubuntu/Debian
  sudo apt install coinor-cbc-dev
  
  # CentOS/RHEL/Fedora
  sudo yum install coin-or-Cbc-devel
  ```

## Usage

The HBCN Constrainer is a Pulsar Half-buffer Channel Network timing analysis tool with three main commands:

### Main Command
```bash
hbcn <COMMAND>
```

### Available Commands

#### 1. `expand` - Convert structural graph to HBCN representation
```bash
hbcn expand [OPTIONS] --output <OUTPUT> <INPUT>
```
- **Description**: Convert a structural graph to HBCN representation and write it to an output file
- **Arguments**:
  - `<INPUT>`: Structural graph input file
- **Required Options**:
  - `-o, --output <OUTPUT>`: HBCN output file
- **Options**:
  - `--forward-completion`: Enable forward completion delay calculation

#### 2. `analyse` - Estimate virtual-delay cycle-time
```bash
hbcn analyse [OPTIONS] <INPUT>
```
- **Description**: Estimate the virtual-delay cycle-time, it can be used to tune the circuit performance. Use `--depth` to analyze cycle depth instead of weighted cycle time.
- **Arguments**:
  - `<INPUT>`: HBCN input file (default) or structural graph input file if `--structural` is passed
- **Options**:
  - `--structural`: Read input as a structural graph instead of an HBCN
  - `--depth`: Perform depth analysis (unweighted) instead of weighted cycle time analysis
  - `-r, --report <REPORT>`: Report file for analysis results (default: stdout)
  - `--vcd <VCD>`: VCD waveform file with virtual-delay arrival times
  - `--dot <DOT>`: DOT file displaying the StructuralHBCN marked graph

#### 3. `constrain` - Constrain the cycle-time
```bash
hbcn constrain [OPTIONS] --sdc <SDC> --cycle-time <CYCLE_TIME> --minimal-delay <MINIMAL_DELAY> <INPUT>
```
- **Description**: Constrain the cycle-time using continuous proportional constraints
- **Arguments**:
  - `<INPUT>`: HBCN input file (default) or structural graph input file if --structural is passed
- **Input Options**:
  - `--structural`: Read input as a structural graph instead of an HBCN
- **Required Options**:
  - `--sdc <SDC>`: Output SDC constraints file
  - `-t, --cycle-time <CYCLE_TIME>`: Cycle-time constraint
  - `-m, --minimal-delay <MINIMAL_DELAY>`: Minimal propagation-path delay
- **Optional Output Options**:
  - `--csv <CSV>`: Output CSV file
  - `--rpt <RPT>`: Output report file
  - `--vcd <VCD>`: Output VCD file with arrival times
- **Algorithm Options**:
  - `--no-proportinal`: Use pseudo-clock to constrain paths
  - `--no-forward-completion`: Don't use forward completion delay if greater than path virtual delay
- **Margin Options**:
  - `-f, --forward-margin <FORWARD_MARGIN>`: Percentual margin between maximum and minimum delay in the forward path
  - `-b, --backward-margin <BACKWARD_MARGIN>`: Minimal percentual margin between maximum and minimum delay in the backward path

### LP Solver Selection

The HBCN Constrainer supports runtime solver selection through environment variables:

#### Runtime Solver Selection
```bash
# Use Gurobi solver (if available)
HBCN_LP_SOLVER=gurobi hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural

# Use Coin CBC solver
HBCN_LP_SOLVER=coin_cbc hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural

# Use default solver (Gurobi if available, otherwise Coin CBC)
hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural

# Use HBCN format (default, no --structural flag needed)
hbcn constrain input.hbcn --sdc output.sdc -t 10.0 -m 1.0
```

#### Supported Solver Names
- `gurobi` - Gurobi commercial solver
- `coin_cbc`, `coin-cbc`, `cbc` - Coin CBC open-source solver

### Example Usage

#### Basic Constraint Generation
```bash
# Generate basic constraints from structural graph
hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural

# Generate basic constraints from HBCN format (default)
hbcn constrain input.hbcn --sdc output.sdc -t 10.0 -m 1.0

# With additional output formats
hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural \
    --csv constraints.csv --rpt analysis.rpt --vcd timing.vcd
```

#### Algorithm Selection
```bash
# Use proportional constraints (default)
hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural

# Use pseudoclock constraints
hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural --no-proportinal
```

#### Margin Control
```bash
# Set forward and backward margins
hbcn constrain input.graph --sdc output.sdc -t 10.0 -m 1.0 --structural \
    --forward-margin 15 --backward-margin 20
```

#### Expansion Command
```bash
# Convert structural graph to HBCN format
hbcn expand input.graph --output circuit.hbcn

# Convert with forward completion delay enabled
hbcn expand input.graph --output circuit.hbcn --forward-completion
```

#### Analysis Commands
```bash
# Estimate cycle-time with VCD output (reads HBCN by default)
hbcn analyse circuit.hbcn --vcd timing.vcd

# Analyse structural graph directly
hbcn analyse input.graph --structural --vcd timing.vcd

# Perform depth analysis (unweighted) instead of weighted cycle time
hbcn analyse input.graph --structural --depth

# Generate DOT graph visualization
hbcn analyse input.graph --structural --dot circuit.dot

# Save analysis report to file
hbcn analyse input.graph --structural --report analysis.rpt
```

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

## HBCN Format

The HBCN (Half-Buffer Channel Network) format is a text-based representation of an HBCN graph. This format is used as an intermediate representation between structural graphs and timing analysis. HBCN files are generated by the `expand` command and can be used directly as input to the `analyse` command.

### Format Structure

Each line in an HBCN file represents a **place** (edge) in the HBCN graph, connecting two transitions (nodes):

```
[token] source_transition => target_transition : delay
```

Where:
- **token**: Optional `*` prefix indicates the place is marked (contains a token). Unmarked places have two spaces for alignment.
- **source_transition**: The source transition of the place
- **target_transition**: The target transition of the place  
- **delay**: Delay constraint(s) for the place

### Transitions

Transitions represent events at circuit nodes and can be:

- **Data transitions**: `+{name}` - Represents a data transition at a circuit node
- **Spacer transitions**: `-{name}` - Represents a spacer/null transition at a circuit node

The transition name identifies the circuit node (port or register). Names are TCL-escaped:
- Braces in names are escaped: `{` becomes `\{` and `}` becomes `\}`
- Example: A node named `my{node}` is written as `+{my\{node\}}`

### Delays

Delay constraints specify timing requirements between transitions:

- **Min-Max Delay**: `(min, max)` when both minimum and maximum delays are specified
  - Example: `(1.0, 2.5)` means delay must be between 1.0 and 2.5 time units
  - Both integer and floating-point values are accepted (e.g., `(1, 2)` or `(1.0, 2.5)`)
- **Max Delay Only**: `max` when only maximum delay is specified (no minimum)
  - Example: `10` or `10.0` means delay must be at most 10 time units
  - Both integer and floating-point values are accepted

### Example HBCN File

```
  +{input} => +{output} : 50.0
  -{input} => -{output} : 50.0
  +{output} => -{input} : 10.0
* -{output} => +{input} : 10.0
```

This example shows:
- Four places connecting transitions at `input` and `output` nodes
- Forward places: `+{input} => +{output}` and `-{input} => -{output}` (data and spacer flows)
- Backward places: `+{output} => -{input}` and `-{output} => +{input}` (acknowledgment paths)
- One marked place (indicated by `*`) representing initial token marking
- Delay constraints: three places with max-only delays (50.0, 50.0, 10.0) and one with a delay of 10.0

### Complex Example

```
* +{port:in} => +{reg1} : (1.0, 2.0)
  +{reg1} => -{port:in} : (0.5, 1.5)
  -{port:in} => -{reg1} : (0.5, 1.0)
  -{reg1} => +{port:in} : (0.0, 1.0)
  +{reg1} => +{reg2} : (2.0, 3.5)
  -{reg2} => -{reg1} : (1.0, 2.0)
```

This shows a more complex circuit with:
- Port nodes (prefixed with `port:`): `port:in`
- Register nodes: `reg1`, `reg2`
- Mixed delay constraints (some with min-max, some max-only)
- Token marking on the initial place

### Usage

HBCN files can be:
- **Generated** from structural graphs using: `hbcn expand input.graph --output circuit.hbcn`
- **Used directly** for analysis: `hbcn analyse circuit.hbcn`
- **Analyzed** with depth mode: `hbcn analyse circuit.hbcn --depth`
- **Visualized** with DOT output: `hbcn analyse circuit.hbcn --dot graph.dot`

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

