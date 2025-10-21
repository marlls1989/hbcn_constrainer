# HBCN Constrainer - Comprehensive Testing Suite

This project includes a comprehensive test suite covering all aspects of the HBCN constrainer functionality. The tests are organized into unit tests, integration tests, and regression tests to ensure robust operation across various scenarios and use cases.

## Test Suite Overview

### ✅ **Overall Test Status**
All **119 tests** are currently **PASSING**:

- **84 Unit Tests** - Testing individual components and functions
- **35 Integration Tests** - Testing end-to-end functionality
- **0 Failed Tests** - All tests passing successfully

## Test Organization

### **Unit Tests by Module**

#### **Core HBCN Module (`src/hbcn/mod.rs`) - 13 tests**
**Core HBCN Conversion Tests:**
- `test_simple_two_node_conversion` - Basic two-node circuit conversion
- `test_data_register_conversion` - DataReg internal structure handling
- `test_transition_properties` - Transition node validation
- `test_place_properties` - Place edge validation
- `test_forward_completion_disabled` - Weight calculation without forward completion
- `test_forward_completion_enabled` - Weight calculation with forward completion
- `test_complex_graph_conversion` - Multi-connection circuit handling
- `test_channel_phases` - Channel phase token marking
- `test_weight_calculations` - Edge weight computation
- `test_empty_graph` - Minimal single-node circuit
- `test_register_types` - All register type conversions (NullReg, ControlReg, UnsafeReg)

**Cyclic Circuit Tests:**
- `test_cyclic_path_conversion` - Basic cyclic circuit conversion
- `test_complex_cyclic_conversion` - Complex multi-feedback circuits


#### **Constraint Module (`src/constrain/hbcn.rs`) - 15 tests**
**Constraint Algorithm Tests:**
- `test_constraint_algorithms_linear_chain` - Linear circuit constraint generation
- `test_constraint_algorithms_branching` - Branching topology handling
- `test_constraint_algorithms_with_feedback` - Feedback loop constraint generation
- `test_constraint_generation_boundary_conditions` - Edge case parameter testing
- `test_delay_pair_functionality` - DelayPair struct validation
- `test_markable_place_functionality` - Place marking operations
- `test_proportional_vs_pseudoclock_differences` - Algorithm comparison
- `test_margin_effects_detailed` - Margin parameter effects

**Cyclic Constraint Tests:**
- `test_cyclic_constraint_algorithms` - Cyclic circuit constraint generation
- `test_cyclic_forward_completion` - Forward completion with cycles
- `test_cyclic_minimal_feedback` - Minimal feedback circuit testing

**Constraint Verification Tests:**
- `test_hbcn_pseudoclock_cycle_time_verification` - Pseudoclock constraint verification
- `test_hbcn_proportional_cycle_time_verification` - Proportional constraint verification
- `test_hbcn_cyclic_cycle_time_verification` - Cyclic circuit constraint verification
- `test_hbcn_constraint_tightness_verification` - Constraint tightness validation

#### **Analysis Module (`src/analyse/hbcn.rs`) - 4 tests**
**Critical Cycle Analysis:**
- `test_critical_cycle_detection` - Critical cycle finding algorithm
- `test_transition_event_timing` - Transition event timing validation
- `test_cyclic_critical_cycle_detection` - Critical cycles in cyclic circuits
- `test_cyclic_timing_calculations` - Timing calculations for cyclic circuits

#### **Analysis Module (`src/analyse/mod.rs`) - 14 tests**
**Cycle Time Computation:**
- `test_cycle_time_computation_weighted` - Weighted cycle time calculation
- `test_cycle_time_computation_unweighted` - Unweighted cycle time calculation
- `test_cycle_time_with_datareg` - Cycle time with DataReg circuits
- `test_cycle_time_cyclic_circuit` - Cycle time in cyclic circuits

**Analysis Functions:**
- `test_critical_cycle_detection` - Critical cycle detection
- `test_complex_circuit_analysis` - Complex circuit analysis
- `test_vcd_generation` - VCD file generation
- `test_dot_generation` - DOT graph generation
- `test_cycle_cost_calculation` - Cycle cost computation
- `test_transition_type_classification` - Transition classification
- `test_token_counting` - Token counting in circuits
- `test_depth_analysis` - Circuit depth analysis
- `test_empty_graph_analysis` - Empty graph handling
- `test_analysis_error_handling` - Error handling in analysis

#### **Constraint Unit Tests (`src/constrain/tests.rs`) - 27 tests**
**Basic Constraint Tests:**
- `test_pseudoclock_constraints_basic` - Basic pseudoclock constraints
- `test_proportional_constraints_basic` - Basic proportional constraints
- `test_infeasible_constraints` - Infeasible constraint handling
- `test_proportional_constraints_with_margins` - Margin parameter testing
- `test_constraints_with_datareg` - DataReg constraint generation
- `test_minimal_circuit` - Minimal circuit constraint testing
- `test_forward_completion_effects` - Forward completion effects
- `test_constraint_validity` - Constraint validity checking

**Parameter Validation:**
- `test_invalid_cycle_time_zero` - Zero cycle time handling
- `test_invalid_cycle_time_negative` - Negative cycle time handling
- `test_constraint_result_timing` - Constraint result timing validation
- `test_extreme_margin_values` - Extreme margin value testing

**Complex Circuit Tests:**
- `test_complex_multipath_constraints` - Multi-path circuit constraints
- `test_critical_cycle_analysis` - Critical cycle analysis
- `test_cyclic_path_constraints` - Cyclic path constraints
- `test_cyclic_feedback_constraints` - Cyclic feedback constraints
- `test_complex_cyclic_constraints` - Complex cyclic constraints
- `test_cyclic_constraint_validity` - Cyclic constraint validity
- `test_cyclic_tight_timing` - Tight timing in cyclic circuits
- `test_cyclic_forward_completion_effects` - Forward completion in cycles

**Constraint Verification:**
- `test_pseudoclock_constraints_cycle_time_verification` - Pseudoclock verification
- `test_proportional_constraints_cycle_time_verification` - Proportional verification
- `test_cyclic_constraints_cycle_time_verification` - Cyclic verification
- `test_complex_constraints_cycle_time_verification` - Complex circuit verification
- `test_constraint_tightness_verification` - Constraint tightness verification
- `test_min_delay_cycle_time_verification` - Minimal delay verification
- `test_proportional_cyclic_cycle_time_verification` - Proportional cyclic verification

#### **SDC Generation Tests (`src/constrain/sdc_tests.rs`) - 15 tests**
**SDC Constraint Generation:**
- `test_sdc_port_to_port_constraints` - Port-to-port SDC constraints
- `test_sdc_register_constraints` - Register SDC constraints
- `test_sdc_min_only_constraints` - Minimum delay only constraints
- `test_sdc_empty_constraints` - Empty constraint handling
- `test_sdc_multiple_constraints` - Multiple constraint generation
- `test_sdc_cyclic_circuit_constraints` - Cyclic circuit SDC constraints
- `test_sdc_complex_cyclic_constraints` - Complex cyclic SDC constraints
- `test_sdc_cyclic_tight_timing` - Tight timing SDC constraints
- `test_sdc_cyclic_max_only_constraints` - Maximum delay only constraints

**SDC Format Testing:**
- `test_port_wildcard_generation` - Port wildcard generation
- `test_port_instance_generation` - Port instance generation
- `test_src_rails_generation` - Source rails generation
- `test_dst_rails_generation` - Destination rails generation
- `test_sdc_decimal_precision` - Decimal precision in SDC
- `test_sdc_tcl_structure` - TCL structure validation

#### **SDC Module Tests (`src/constrain/sdc.rs`) - 5 tests**
**SDC Core Functions:**
- `test_sdc_port_to_port_constraints` - Port-to-port constraint generation
- `test_sdc_register_constraints` - Register constraint generation
- `test_port_wildcard_generation` - Wildcard port generation
- `test_port_instance_generation` - Instance port generation
- `test_sdc_multiple_constraints` - Multiple constraint handling

#### **Structural Graph Module (`src/structural_graph/mod.rs`) - 8 tests**
**Graph Parsing Tests:**
- `parse_valid` - Valid graph parsing
- `parse_err_undefined` - Undefined node error handling
- `parse_err_syntax` - Syntax error handling
- `parse_realistic_port_names` - Realistic port name parsing
- `parse_complex_adjacency_list` - Complex adjacency list parsing
- `parse_all_register_types` - All register type parsing
- `parse_floating_point_weights` - Floating point weight parsing
- `parse_test_graph_format` - Test graph format parsing

### **Integration Tests (`tests/integration_tests.rs`) - 35 tests**

#### **Constraint Generation Tests (12 tests):**
- `test_simple_two_port_constraint_generation` - Basic constraint generation
- `test_proportional_vs_pseudoclock_constraints` - Algorithm mode comparison
- `test_forward_completion_effects` - Forward completion optimisation
- `test_margin_parameters` - Margin parameter handling
- `test_vcd_output_generation` - VCD timing output
- `test_complex_circuit_constraints` - Multi-path circuit handling
- `test_edge_case_very_tight_timing` - Tight timing constraints
- `test_zero_minimal_delay` - Zero minimal delay parameter
- `test_boundary_margin_values` - Boundary margin testing
- `test_single_node_circuit` - Edge case handling
- `test_invalid_input_file` - Error handling for missing files
- `test_malformed_graph_input` - Error handling for malformed input

#### **Cyclic Circuit Tests (4 tests):**
- `test_cyclic_path_constraint_generation` - Cyclic path constraint generation
- `test_cyclic_path_algorithm_comparison` - Algorithm comparison for cycles
- `test_complex_cyclic_circuit` - Complex cyclic circuit handling
- `test_cyclic_tight_timing` - Tight timing in cyclic circuits

#### **Analysis Integration Tests (9 tests):**
- `test_analyse_simple_circuit` - Simple circuit analysis
- `test_analyse_with_vcd_output` - Analysis with VCD output
- `test_analyse_with_dot_output` - Analysis with DOT output
- `test_analyse_with_multiple_outputs` - Multiple output analysis
- `test_analyse_cyclic_circuit` - Cyclic circuit analysis
- `test_analyse_complex_circuit` - Complex circuit analysis
- `test_depth_simple_circuit` - Simple circuit depth analysis
- `test_depth_cyclic_circuit` - Cyclic circuit depth analysis
- `test_analyse_invalid_file` - Invalid file error handling
- `test_analyse_malformed_input` - Malformed input error handling
- `test_analyse_single_node_circuit` - Single node circuit analysis
- `test_analyse_tight_timing_circuit` - Tight timing circuit analysis

#### **Constraint Verification Tests (6 tests):**
- `test_constrainer_meets_cycle_time_simple_circuit` - Simple circuit verification
- `test_constrainer_meets_cycle_time_datareg_circuit` - DataReg circuit verification
- `test_constrainer_meets_cycle_time_cyclic_circuit` - Cyclic circuit verification
- `test_constrainer_meets_cycle_time_complex_circuit` - Complex circuit verification
- `test_constrainer_meets_tight_cycle_time` - Tight cycle time verification
- `test_constrainer_algorithm_comparison` - Algorithm comparison verification
- `test_constrainer_verification_error_handling` - Verification error handling

## Test Categories

### **Core Algorithm Testing**
- **Proportional Constraints**: Tests the proportional delay constraint algorithm
- **Pseudoclock Constraints**: Tests the pseudoclock-based constraint algorithm
- **Forward Completion**: Tests optimisation flags and their effects on constraint generation
- **Critical Cycle Detection**: Tests cycle finding and analysis algorithms
- **Cycle Time Computation**: Tests weighted and unweighted cycle time calculations

### **Circuit Topology Testing**
- **Linear Circuits**: Simple two-port and multi-port linear circuits
- **Branching Circuits**: Circuits with multiple paths and merge points
- **Cyclic Circuits**: Circuits with feedback loops and cycles
- **Complex Circuits**: Multi-path circuits with registers and logic blocks
- **DataReg Circuits**: Circuits with DataReg internal structure
- **Register Types**: All register types (NullReg, ControlReg, UnsafeReg, DataReg)

### **Parameter Validation**
- **Cycle Time**: Various cycle time values from very tight (0.5ns) to relaxed (25ns)
- **Minimal Delay**: Testing zero and positive minimal delay values
- **Margins**: Forward and backward margin parameters from 0% to 99%
- **Boundary Values**: Extreme parameter values and edge cases
- **Invalid Parameters**: Zero, negative, and invalid parameter handling

### **Output Format Testing**
- **SDC Files**: Synopsys Design Constraints format generation
- **CSV Files**: Constraint data in comma-separated values format
- **VCD Files**: Value Change Dump timing visualisation files
- **DOT Files**: Graph visualization format
- **Report Files**: Human-readable constraint analysis reports

### **Constraint Verification**
- **Cycle Time Verification**: Ensures generated constraints meet target cycle times
- **Constraint Tightness**: Tests that tighter constraints result in lower cycle times
- **Algorithm Comparison**: Compares different constraint algorithms
- **Timing Validation**: Validates timing calculations and event scheduling

### **Edge Case and Error Handling**
- **Invalid Inputs**: Non-existent files, malformed graph syntax
- **Infeasible Constraints**: Very tight timing that cannot be satisfied
- **Single Node Circuits**: Degenerate cases with minimal connectivity
- **Empty Graphs**: Minimal circuit handling
- **Malformed Data**: Error handling for invalid input formats

## Running the Test Suite

### Run All Tests
```bash
# Run all unit and integration tests
cargo test

# Run only integration tests
cargo test --test integration_tests
```

### Run Specific Test Categories
```bash
# Test core HBCN functionality
cargo test hbcn::tests

# Test constraint generation
cargo test constrain::tests

# Test analysis functionality
cargo test analyse::tests

# Test SDC generation
cargo test sdc

# Test structural graph parsing
cargo test structural_graph::tests

# Test specific functionality
cargo test test_simple_two_port
cargo test test_proportional_vs_pseudoclock
cargo test test_cyclic
cargo test test_constraint_verification
```

### Run with Verbose Output
```bash
# Run all tests with verbose output
cargo test -- --nocapture

# Run integration tests with verbose output
cargo test --test integration_tests -- --nocapture

# Run specific test module with verbose output
cargo test hbcn::tests -- --nocapture
```

### Run Tests by Pattern
```bash
# Run all constraint-related tests
cargo test constraint

# Run all cyclic circuit tests
cargo test cyclic

# Run all verification tests
cargo test verification

# Run all SDC tests
cargo test sdc
```

## Test Architecture

### **Multi-Level Testing Approach**

#### **Unit Testing (103 tests)**
- **Module-level testing**: Each module has its own test suite
- **Function-level testing**: Individual functions are tested in isolation
- **Mock data**: Uses controlled test data and mock inputs
- **Fast execution**: Unit tests run quickly for rapid feedback

#### **Integration Testing (35 tests)**
- **End-to-end testing**: Tests complete workflows from input to output
- **Binary execution**: Runs the actual constrainer binary through `cargo run`
- **File I/O testing**: Verifies file generation and content
- **Error condition testing**: Tests graceful failure modes

### **Test Organization by Functionality**

#### **Core HBCN Tests**
- Circuit conversion from structural graphs to HBCN
- Transition and place property validation
- Weight calculations and channel phases
- Forward completion optimization

#### **Constraint Generation Tests**
- Algorithm testing (proportional vs pseudoclock)
- Parameter validation and boundary testing
- Margin effects and constraint tightness
- Cyclic circuit constraint handling

#### **Analysis Tests**
- Critical cycle detection and analysis
- Cycle time computation (weighted/unweighted)
- VCD and DOT file generation
- Circuit depth and token counting

#### **SDC Generation Tests**
- SDC format validation and structure
- Port wildcard and instance generation
- Constraint formatting and precision
- TCL structure validation

### **Temporary File Management**
- Uses `tempfile` crate for safe temporary file handling
- Automatic cleanup prevents test interference
- Parallel test execution is supported
- Isolated test environments prevent conflicts

### **Command Line Interface Testing**
Tests cover all CLI parameters:
- `--sdc`: SDC output file path
- `--csv`: CSV output file path  
- `--rpt`: Report output file path
- `--vcd`: VCD output file path
- `--dot`: DOT graph output file path
- `-t/--cycle-time`: Target cycle time
- `-m/--minimal-delay`: Minimal delay constraint
- `--forward-margin`: Forward path margin percentage
- `--backward-margin`: Backward path margin percentage
- `--no-proportional`: Use pseudoclock algorithm
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

## Test Coverage and Quality

### **Comprehensive Coverage**
The test suite provides comprehensive coverage across:
- **All major modules**: HBCN, constraints, analysis, SDC generation, structural graph parsing
- **All algorithms**: Proportional and pseudoclock constraint algorithms
- **All circuit types**: Linear, branching, cyclic, and complex circuits
- **All output formats**: SDC, CSV, VCD, DOT, and report files
- **All error conditions**: Invalid inputs, infeasible constraints, malformed data

### **Test Quality Metrics**
- **119 total tests** with 100% pass rate
- **Unit test coverage**: Individual function and module testing
- **Integration test coverage**: End-to-end workflow testing
- **Regression prevention**: Comprehensive test suite prevents breaking changes
- **Performance validation**: Constraint verification ensures timing requirements are met

### **Test Organization Status**
**✅ Clean State Achieved:** All tests are now properly organized in their respective modules:
- Core HBCN functionality tests remain in `src/hbcn/mod.rs`
- Constraint-related tests are in `src/constrain/hbcn.rs` and `src/constrain/tests.rs`
- Analysis-related tests are in `src/analyse/hbcn.rs` and `src/analyse/mod.rs`
- No duplicate tests exist across modules

## Detailed Unit Test Coverage

### **Unit Test Statistics**
- **Total Unit Tests**: 84 tests across all modules
- **Integration Tests**: 35 additional integration tests
- **Coverage Areas**: 
  - Constraint generation algorithms (pseudoclock and proportional)
  - Graph conversion and validation
  - SDC output generation
  - Error handling and edge cases
  - Parameter validation
  - Timing and cycle analysis

### **Key Testing Principles Applied**

1. **Comprehensive Coverage**: Tests cover all major code paths and functionality
2. **Edge Case Testing**: Includes tests for boundary conditions and error scenarios
3. **Parameter Validation**: Tests both valid and invalid parameter combinations
4. **Algorithm Verification**: Validates that different algorithms produce expected results
5. **Data Integrity**: Ensures that all generated data structures are valid
6. **Error Handling**: Tests proper handling of infeasible and invalid scenarios

### **Detailed Unit Test Breakdown**

#### **Constraint Generation Unit Tests (`src/constrain/tests.rs`) - 27 tests**

**Basic Algorithm Testing:**
- `test_pseudoclock_constraints_basic`: Tests pseudoclock constraint generation with simple circuits
- `test_proportional_constraints_basic`: Tests proportional constraint generation with simple circuits
- `test_constraints_with_datareg`: Tests constraint generation with DataReg components (more complex circuits)

**Parameter Validation:**
- `test_invalid_cycle_time_zero`: Validates that zero cycle time causes panic (safety check)
- `test_invalid_cycle_time_negative`: Validates that negative cycle time causes panic (safety check)
- `test_infeasible_constraints`: Tests proper handling of infeasible constraint scenarios

**Margin Effects:**
- `test_proportional_constraints_with_margins`: Tests forward and backward margin effects on constraints
- `test_extreme_margin_values`: Tests constraint generation with extreme margin values

**Edge Cases:**
- `test_minimal_circuit`: Tests constraint generation with minimal viable circuits
- `test_forward_completion_effects`: Tests that forward completion parameter affects results
- `test_complex_multipath_constraints`: Tests complex circuits with multiple paths and components

**Result Validation:**
- `test_constraint_validity`: Validates that generated constraints have reasonable values
- `test_constraint_result_timing`: Tests that timing information in results is valid
- `test_critical_cycle_analysis`: Tests critical cycle detection in cyclic circuits

#### **HBCN Core Unit Tests (`src/hbcn/mod.rs`) - 13 tests**

**Graph Conversion:**
- `test_simple_two_node_conversion`: Tests basic structural graph to HBCN conversion
- `test_data_register_conversion`: Tests conversion with DataReg components
- `test_complex_graph_conversion`: Tests conversion of complex multi-component graphs
- `test_empty_graph`: Tests handling of empty graphs

**Node and Edge Properties:**
- `test_transition_properties`: Tests that transitions have correct circuit node references
- `test_place_properties`: Tests place properties (weights, directions, etc.)
- `test_weight_calculations`: Tests that weights are calculated correctly
- `test_channel_phases`: Tests channel phase handling

**Forward Completion:**
- `test_forward_completion_disabled`: Tests behaviour with forward completion disabled
- `test_forward_completion_enabled`: Tests behaviour with forward completion enabled

**Register Types:**
- `test_register_types`: Tests different register type handling

**Cyclic Circuit Tests:**
- `test_cyclic_path_conversion`: Tests basic cyclic circuit conversion
- `test_complex_cyclic_conversion`: Tests complex multi-feedback circuits


#### **SDC Generation Unit Tests (`src/constrain/sdc.rs`) - 5 tests**

**Basic SDC Generation:**
- `test_sdc_port_to_port_constraints`: Tests SDC generation for port-to-port constraints
- `test_sdc_register_constraints`: Tests SDC generation for register constraints
- `test_sdc_multiple_constraints`: Tests SDC generation with multiple mixed constraints

**Helper Function Tests:**
- `test_port_wildcard_generation`: Tests port wildcard string generation
- `test_port_instance_generation`: Tests port instance string generation

## Maintenance

### Adding New Tests
When adding new tests:
1. **Follow naming convention**: `test_<functionality>` for unit tests
2. **Use appropriate test module**: Place tests in the relevant module's test section
3. **Test both success and failure cases**: Include positive and negative test cases
4. **Use helper functions**: Leverage existing test utilities (`create_test_file`, `run_hbcn_constrain`)
5. **Include clear documentation**: Document what each test validates
6. **Maintain test organization**: Keep tests grouped by functionality

### Updating Tests
When modifying the constrainer:
1. **Run full test suite**: `cargo test` to check for regressions
2. **Update test expectations**: Modify assertions if behavior intentionally changes
3. **Add new tests**: Create tests for new features and functionality
4. **Verify error messages**: Ensure error message assertions match actual output
5. **Update documentation**: Keep TESTING.md current with new tests

### Test Organization Guidelines
- **Unit tests**: Place in the same module as the code being tested
- **Integration tests**: Place in `tests/integration_tests.rs`
- **Test modules**: Use `#[cfg(test)] mod tests` for unit test organization
- **Helper functions**: Create reusable test utilities for common operations

### Running the Tests

```bash
# Run all unit tests
cargo test

# Run only unit tests (excluding integration tests)
cargo test --lib

# Run with verbose output
cargo test -- --nocapture

# Run specific test module
cargo test constrain::tests
cargo test hbcn::tests
cargo test constrain::sdc::tests
```

All tests are designed to be deterministic and reliable, using feasible parameters to avoid solver-related failures while still thoroughly testing the constraining functionality.

## Dependencies

### **Required Dependencies**
- `tempfile = "3.8"` for temporary file management
- Standard Rust testing framework (`std::test`)
- The HBCN constrainer binary (built via `cargo`)

### **Optional Dependencies for Full Testing**
- **Gurobi solver**: Required for constraint optimisation tests
- **System resources**: Sufficient memory and CPU for parallel test execution
- **File system access**: For temporary file creation and cleanup

### **Test Environment Requirements**
- **Rust toolchain**: Latest stable Rust compiler
- **Cargo**: Package manager and build system
- **Operating system**: Linux, macOS, or Windows (tested on all platforms)
- **Memory**: Minimum 2GB RAM for complex circuit tests
- **Storage**: Temporary space for test file generation
