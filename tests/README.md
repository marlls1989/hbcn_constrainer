# HBCN Constrainer - Regression Test Suite

This directory contains comprehensive regression tests for the HBCN constrainer functionality. The tests are designed to verify that the constraining algorithms work correctly across various scenarios and edge cases.

## Test Structure

### Integration Tests (`integration_tests.rs`)

These tests run the constrainer as a black-box through the command line interface, ensuring end-to-end functionality works correctly.

#### Test Categories

1. **Basic Functionality Tests**
   - `test_simple_two_port_constraint_generation`: Tests basic constraint generation with a minimal two-port circuit
   - `test_complex_circuit_constraints`: Tests a more complex circuit with multiple registers and paths

2. **Algorithm Mode Tests**
   - `test_proportional_vs_pseudoclock_constraints`: Compares proportional and pseudoclock constraint algorithms
   - `test_forward_completion_effects`: Tests the forward completion optimisation flag

3. **Parameter Validation Tests**
   - `test_margin_parameters`: Tests forward and backward margin parameter handling
   - `test_boundary_margin_values`: Tests boundary values (0% and 99% margins)
   - `test_zero_minimal_delay`: Tests zero minimal delay parameter

4. **Output Format Tests**
   - `test_vcd_output_generation`: Tests VCD timing output generation
   - Tests for SDC, CSV, and report file generation

5. **Edge Case Tests**
   - `test_edge_case_very_tight_timing`: Tests behaviour with very tight timing constraints
   - `test_single_node_circuit`: Tests single node circuit handling
   - `test_invalid_input_file`: Tests error handling with missing input files
   - `test_malformed_graph_input`: Tests error handling with malformed input

## Test Coverage

The regression tests cover:

- **Core Algorithms**: Both proportional and pseudoclock constraint generation methods
- **Input/Output**: All supported file formats (graph, SDC, CSV, VCD, report)
- **Parameter Handling**: All command-line parameters and their boundary values
- **Error Handling**: Graceful handling of invalid inputs and infeasible constraints
- **Edge Cases**: Unusual but valid circuit topologies and parameter combinations

## Running Tests

### Run All Integration Tests
```bash
cargo test --test integration_tests
```

### Run Specific Test
```bash
cargo test --test integration_tests test_simple_two_port_constraint_generation
```

### Run Tests with Output
```bash
cargo test --test integration_tests -- --nocapture
```

## Test Data

Tests use temporary files and directories created with the `tempfile` crate to ensure:
- No interference between tests
- Automatic cleanup
- Parallel test execution safety

### Sample Circuit Formats

The tests use various circuit description formats:

#### Simple Two-Port Circuit
```
Port "a" [("b", 20)]
Port "b" []
```

#### Circuit with Data Registers
```
Port "input" [("reg", 50)]
DataReg "reg" [("output", 75)]
Port "output" []
```

#### Complex Multi-Path Circuit
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

## Expected Behaviour

### Successful Constraint Generation
- SDC files contain `create_clock` commands with correct periods
- CSV files have proper headers and constraint data
- Report files contain cycle analysis information
- VCD files contain valid VCD format markers

### Error Handling
- Invalid input files produce appropriate error messages
- Malformed graph syntax is handled gracefully
- Infeasible timing constraints are detected and reported
- Missing required parameters cause clear error messages

## Adding New Tests

When adding new regression tests:

1. Create a descriptive test function name starting with `test_`
2. Use `create_test_file()` helper for temporary input files
3. Use `run_hbcn_constrain()` helper to execute the constrainer
4. Assert on both success/failure and output file contents
5. Include clear documentation about what the test verifies

### Test Template
```rust
#[test]  
fn test_new_functionality() {
    let graph_content = r#"
        // Your circuit description here
    "#;
    
    let (_temp_dir, input_path) = create_test_file(graph_content);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output.sdc");
    
    let output = run_hbcn_constrain(
        &input_path,
        &output_path,
        10.0, // cycle_time
        1.0,  // minimal_delay
        vec!["--additional", "args"],
    ).expect("Failed to run test");
    
    assert!(output.status.success(), "Should succeed");
    assert!(output_path.exists(), "Output should be generated");
    
    // Add specific assertions for your test case
}
```

## Maintenance Notes

- Tests should be kept up-to-date with CLI argument changes
- Test input files should represent realistic circuit topologies
- Error message assertions should be updated if error formatting changes
- Performance-sensitive tests should have reasonable timeouts
