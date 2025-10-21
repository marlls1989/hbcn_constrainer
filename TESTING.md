# HBCN Constrainer - Testing and Regression Suite

This project now includes a comprehensive regression test suite for the constraining functionality. The tests ensure that the HBCN constrainer works correctly across various scenarios and use cases.

## Regression Test Coverage

### ✅ **Test Suite Status**
All 12 regression tests are currently **PASSING**:

- ✅ `test_simple_two_port_constraint_generation` - Basic constraint generation
- ✅ `test_proportional_vs_pseudoclock_constraints` - Algorithm mode comparison
- ✅ `test_forward_completion_effects` - Forward completion optimisation
- ✅ `test_margin_parameters` - Margin parameter handling
- ✅ `test_vcd_output_generation` - VCD timing output
- ✅ `test_complex_circuit_constraints` - Multi-path circuit handling
- ✅ `test_edge_case_very_tight_timing` - Tight timing constraints
- ✅ `test_zero_minimal_delay` - Zero minimal delay parameter
- ✅ `test_boundary_margin_values` - Boundary margin testing
- ✅ `test_single_node_circuit` - Edge case handling
- ✅ `test_invalid_input_file` - Error handling for missing files
- ✅ `test_malformed_graph_input` - Error handling for malformed input

## Test Categories

### **Core Algorithm Testing**
- **Proportional Constraints**: Tests the proportional delay constraint algorithm
- **Pseudoclock Constraints**: Tests the pseudoclock-based constraint algorithm
- **Forward Completion**: Tests optimisation flags and their effects on constraint generation

### **Parameter Validation**
- **Cycle Time**: Various cycle time values from very tight (0.5ns) to relaxed (25ns)
- **Minimal Delay**: Testing zero and positive minimal delay values
- **Margins**: Forward and backward margin parameters from 0% to 99%

### **Output Format Testing**
- **SDC Files**: Synopsys Design Constraints format generation
- **CSV Files**: Constraint data in comma-separated values format
- **VCD Files**: Value Change Dump timing visualisation files
- **Report Files**: Human-readable constraint analysis reports

### **Edge Case and Error Handling**
- **Invalid Inputs**: Non-existent files, malformed graph syntax
- **Infeasible Constraints**: Very tight timing that cannot be satisfied
- **Single Node Circuits**: Degenerate cases with minimal connectivity
- **Complex Topologies**: Multi-path circuits with registers and logic blocks

## Running the Test Suite

### Run All Tests
```bash
cargo test --test integration_tests
```

### Run Specific Test Categories
```bash
# Test basic functionality
cargo test --test integration_tests test_simple_two_port

# Test algorithm modes
cargo test --test integration_tests test_proportional_vs_pseudoclock

# Test error handling
cargo test --test integration_tests test_invalid
cargo test --test integration_tests test_malformed
```

### Run with Verbose Output
```bash
cargo test --test integration_tests -- --nocapture
```

## Test Architecture

### **Integration Testing Approach**
The test suite uses **black-box integration testing** by:
- Running the constrainer binary through `cargo run`
- Providing various graph input files
- Verifying output file generation and content
- Testing error conditions and graceful failure

### **Temporary File Management**
- Uses `tempfile` crate for safe temporary file handling
- Automatic cleanup prevents test interference
- Parallel test execution is supported

### **Command Line Interface Testing**
Tests cover all CLI parameters:
- `--sdc`: SDC output file path
- `--csv`: CSV output file path  
- `--rpt`: Report output file path
- `--vcd`: VCD output file path
- `-t/--cycle-time`: Target cycle time
- `-m/--minimal-delay`: Minimal delay constraint
- `--forward-margin`: Forward path margin percentage
- `--backward-margin`: Backward path margin percentage
- `--no-proportinal`: Use pseudoclock algorithm
- `--no-forward-completion`: Disable forward completion optimisation

## Test Input Formats

### Simple Two-Port Circuit
```
Port "a" [("b", 20)]
Port "b" []
```

### Circuit with Data Registers
```
Port "input" [("reg", 50)]
DataReg "reg" [("output", 75)]
Port "output" []
```

### Complex Multi-Path Circuit
```
Port "clk" [("reg1", 5), ("reg2", 5), ("reg3", 5)]
Port "input_a" [("reg1", 45)]
Port "input_b" [("reg2", 55)]
DataReg "reg1" [("logic", 30)]
DataReg "reg2" [("logic", 25)]
DataReg "reg3" [("output", 40)]
DataReg "logic" [("reg3", 35)]
Port "output" []
```

## Continuous Integration

The test suite is designed to:
- **Detect Regressions**: Any changes that break existing functionality
- **Validate New Features**: Ensure new additions work correctly
- **Test Error Handling**: Verify graceful failure modes
- **Performance Monitoring**: Detect significant performance changes

## Expected Test Behaviours

### **Successful Scenarios**
- Generate valid SDC, CSV, VCD, and report files
- Handle various circuit topologies correctly
- Apply timing constraints appropriately
- Optimise using forward completion when enabled

### **Expected Failures**
- **Infeasible Constraints**: Very tight timing or extreme margin values
- **Invalid Inputs**: Non-existent files or malformed graph syntax
- **Solver Issues**: Missing Gurobi solver or optimisation failures

### **Error Handling**
- Graceful failure with informative error messages
- Proper exit codes for different error types
- No crashes or undefined behaviour

## Maintenance

### Adding New Tests
When adding new tests:
1. Follow the existing test naming convention (`test_<functionality>`)
2. Use helper functions (`create_test_file`, `run_hbcn_constrain`)
3. Test both success and failure cases where appropriate
4. Include clear documentation about what is being tested

### Updating Tests
When modifying the constrainer:
1. Run the full test suite to check for regressions
2. Update test expectations if behaviour intentionally changes
3. Add new tests for new features
4. Ensure error message assertions match actual output

## Dependencies

The test suite requires:
- `tempfile = "3.8"` for temporary file management
- Standard Rust testing framework
- The HBCN constrainer binary (built via `cargo`)

For optimal test performance:
- Gurobi solver (for constraint optimisation)
- Sufficient system resources for parallel test execution
