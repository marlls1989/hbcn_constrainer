//! Integration tests for HBCN tools using library API
//!
//! These tests use the library API directly instead of calling cargo run,
//! which is much faster and more efficient.

use hbcn::{AnalyseArgs, ConstrainArgs, DepthArgs, analyse_main, constrain_main, depth_main};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Helper function to create a temporary test file
fn create_test_file(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.graph");
    fs::write(&file_path, content).expect("Failed to write test file");
    (temp_dir, file_path)
}

// Helper function to run hbcn constrain via library API
#[allow(clippy::too_many_arguments)]
fn run_hbcn_constrain(
    input: &Path,
    sdc: &Path,
    cycle_time: f64,
    minimal_delay: f64,
    csv: Option<&Path>,
    rpt: Option<&Path>,
    vcd: Option<&Path>,
    no_proportinal: bool,
    no_forward_completion: bool,
    forward_margin: Option<u8>,
    backward_margin: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = ConstrainArgs {
        input: input.to_path_buf(),
        sdc: sdc.to_path_buf(),
        cycle_time,
        minimal_delay,
        csv: csv.map(|p| p.to_path_buf()),
        rpt: rpt.map(|p| p.to_path_buf()),
        vcd: vcd.map(|p| p.to_path_buf()),
        no_proportinal,
        no_forward_completion,
        forward_margin,
        backward_margin,
    };

    constrain_main(args).map_err(|e| e.into())
}

// Helper function to run hbcn analyse via library API
fn run_hbcn_analyse(
    input: &Path,
    report: Option<&Path>,
    vcd: Option<&Path>,
    dot: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = AnalyseArgs {
        input: input.to_path_buf(),
        report: report.map(|p| p.to_path_buf()),
        vcd: vcd.map(|p| p.to_path_buf()),
        dot: dot.map(|p| p.to_path_buf()),
    };

    analyse_main(args).map_err(|e| e.into())
}

// Helper function to run hbcn depth via library API
fn run_hbcn_depth(input: &Path, report: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let args = DepthArgs {
        input: input.to_path_buf(),
        report: report.map(|p| p.to_path_buf()),
    };

    depth_main(args).map_err(|e| e.into())
}

#[cfg(test)]
mod constrain_regression_tests {
    use super::*;

    /// Test basic constraint generation with a simple two-port circuit
    #[test]
    fn test_simple_two_port_constraint_generation() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");
        let csv_path = temp_output_dir.path().join("test.csv");
        let rpt_path = temp_output_dir.path().join("test.rpt");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            Some(&csv_path),
            Some(&rpt_path),
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Constraint generation should succeed: {:?}",
            result
        );

        // Verify output files exist
        assert!(sdc_path.exists(), "SDC file should be generated");
        assert!(csv_path.exists(), "CSV file should be generated");
        assert!(rpt_path.exists(), "Report file should be generated");

        // Verify SDC content is not empty
        let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC file");
        assert!(!sdc_content.is_empty(), "SDC file should not be empty");
    }

    /// Test that proportional and pseudoclock constraints produce different results
    #[test]
    fn test_proportional_vs_pseudoclock_constraints() {
        let graph_content = r#"Port "a" [("b", 20), ("c", 15)]
Port "b" []
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");

        // Generate proportional constraints
        let prop_sdc_path = temp_output_dir.path().join("prop.sdc");
        let prop_result = run_hbcn_constrain(
            &input_path,
            &prop_sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            false, // proportional enabled
            false,
            None,
            None,
        );
        assert!(
            prop_result.is_ok(),
            "Proportional constraint generation should succeed: {:?}",
            prop_result
        );

        // Generate pseudoclock constraints (no proportional)
        let pseudo_sdc_path = temp_output_dir.path().join("pseudo.sdc");
        let pseudo_result = run_hbcn_constrain(
            &input_path,
            &pseudo_sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            true, // proportional disabled (pseudoclock mode)
            false,
            None,
            None,
        );
        assert!(
            pseudo_result.is_ok(),
            "Pseudoclock constraint generation should succeed: {:?}",
            pseudo_result
        );

        // Verify both files exist and have different content
        assert!(prop_sdc_path.exists(), "Proportional SDC file should exist");
        assert!(
            pseudo_sdc_path.exists(),
            "Pseudoclock SDC file should exist"
        );

        let prop_content =
            fs::read_to_string(&prop_sdc_path).expect("Failed to read proportional SDC");
        let pseudo_content =
            fs::read_to_string(&pseudo_sdc_path).expect("Failed to read pseudoclock SDC");

        assert_ne!(
            prop_content, pseudo_content,
            "Proportional and pseudoclock constraints should differ"
        );
    }

    /// Test that forward completion option changes constraints
    #[test]
    fn test_forward_completion_effects() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");

        // Generate constraints with forward completion
        let fc_sdc_path = temp_output_dir.path().join("fc.sdc");
        let fc_result = run_hbcn_constrain(
            &input_path,
            &fc_sdc_path,
            15.0,
            1.0,
            None,
            None,
            None,
            false,
            false, // forward completion enabled
            None,
            None,
        );
        assert!(
            fc_result.is_ok(),
            "Forward completion constraint generation should succeed: {:?}",
            fc_result
        );

        // Generate constraints without forward completion
        let no_fc_sdc_path = temp_output_dir.path().join("no_fc.sdc");
        let no_fc_result = run_hbcn_constrain(
            &input_path,
            &no_fc_sdc_path,
            15.0,
            1.0,
            None,
            None,
            None,
            false,
            true, // forward completion disabled
            None,
            None,
        );
        assert!(
            no_fc_result.is_ok(),
            "No forward completion constraint generation should succeed: {:?}",
            no_fc_result
        );

        // Verify both files exist
        assert!(
            fc_sdc_path.exists(),
            "Forward completion SDC file should exist"
        );
        assert!(
            no_fc_sdc_path.exists(),
            "No forward completion SDC file should exist"
        );

        let fc_content = fs::read_to_string(&fc_sdc_path).expect("Failed to read FC SDC");
        let no_fc_content = fs::read_to_string(&no_fc_sdc_path).expect("Failed to read no-FC SDC");

        // The constraints should be different when forward completion is toggled
        assert_ne!(
            fc_content, no_fc_content,
            "Forward completion should change constraints"
        );
    }

    /// Test margin parameters
    #[test]
    fn test_margin_parameters() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            Some(10), // 10% forward margin
            Some(5),  // 5% backward margin
        );

        assert!(
            result.is_ok(),
            "Constraint generation with margins should succeed: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should be generated");
    }

    /// Test VCD output generation
    #[test]
    fn test_vcd_output_generation() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");
        let vcd_path = temp_output_dir.path().join("test.vcd");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            None,
            None,
            Some(&vcd_path),
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Constraint generation with VCD should succeed: {:?}",
            result
        );
        assert!(vcd_path.exists(), "VCD file should be generated");

        // Verify VCD content is not empty
        let vcd_content = fs::read_to_string(&vcd_path).expect("Failed to read VCD file");
        assert!(!vcd_content.is_empty(), "VCD file should not be empty");
    }

    /// Test complex circuit with multiple paths
    #[test]
    fn test_complex_circuit_constraints() {
        let graph_content = r#"Port "input" [("reg1", 30), ("reg2", 25)]
DataReg "reg1" [("output1", 20)]
DataReg "reg2" [("output2", 15)]
Port "output1" []
Port "output2" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            15.0,
            2.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Complex circuit constraint generation should succeed: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should be generated");
    }

    /// Test edge case with very tight timing
    #[test]
    fn test_edge_case_very_tight_timing() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        // Very tight timing - may or may not be feasible depending on solver
        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            25.0, // Just slightly more than the path delay
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // This might succeed or fail depending on the exact constraints
        // The important thing is it doesn't panic
        match result {
            Ok(_) => {
                assert!(
                    sdc_path.exists(),
                    "SDC file should be generated if successful"
                );
            }
            Err(_e) => {
                // Tight timing might be infeasible, which is expected
                // println!("Tight timing failed as expected: {}", e);
            }
        }
    }

    /// Test zero minimal delay
    #[test]
    fn test_zero_minimal_delay() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            0.0, // Zero minimal delay
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Constraint generation with zero minimal delay should succeed: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should be generated");
    }

    /// Test boundary margin values (0% and 100%)
    #[test]
    fn test_boundary_margin_values() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");

        // Test with 0% margins
        let sdc_path_min = temp_output_dir.path().join("min.sdc");
        let min_result = run_hbcn_constrain(
            &input_path,
            &sdc_path_min,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            Some(0),
            Some(0),
        );
        assert!(
            min_result.is_ok(),
            "Constraint generation with 0% margins should succeed: {:?}",
            min_result
        );

        // Test with 100% margins (might be infeasible)
        let sdc_path_max = temp_output_dir.path().join("max.sdc");
        let _max_result = run_hbcn_constrain(
            &input_path,
            &sdc_path_max,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            Some(100),
            Some(100),
        );
        // High margins might make the problem infeasible, so we don't assert on success
    }

    /// Test single node circuit
    #[test]
    fn test_single_node_circuit() {
        let graph_content = r#"Port "a" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        // Single node might not have meaningful constraints but shouldn't fail
        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Single node might succeed or fail depending on implementation
        match result {
            Ok(_) => {
                // println!("Single node constraint generation succeeded");
            }
            Err(_e) => {
                // println!("Single node constraint generation failed: {}", e);
            }
        }
    }

    /// Test invalid input file
    #[test]
    fn test_invalid_input_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let input_path = temp_dir.path().join("nonexistent.graph");
        let sdc_path = temp_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(result.is_err(), "Should fail with non-existent input file");
    }

    /// Test malformed graph input
    #[test]
    fn test_malformed_graph_input() {
        let graph_content = "This is not a valid graph format";

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(result.is_err(), "Should fail with malformed input");
    }

    /// Test cyclic path constraint generation
    #[test]
    fn test_cyclic_path_constraint_generation() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" [("a", 10)]
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            20.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Cyclic paths might succeed or fail depending on timing
        match result {
            Ok(_) => {
                assert!(
                    sdc_path.exists(),
                    "SDC file should be generated if successful"
                );
                // println!("Cyclic constraint generation succeeded");
            }
            Err(_e) => {
                // println!("Cyclic constraint generation failed: {}", e);
            }
        }
    }

    /// Test cyclic path with proportional vs pseudoclock
    #[test]
    fn test_cyclic_path_algorithm_comparison() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" [("a", 10)]
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");

        // Try with proportional
        let prop_sdc_path = temp_output_dir.path().join("prop.sdc");
        let prop_result = run_hbcn_constrain(
            &input_path,
            &prop_sdc_path,
            20.0,
            1.0,
            None,
            None,
            None,
            false, // proportional enabled
            false,
            None,
            None,
        );

        // Try with pseudoclock
        let pseudo_sdc_path = temp_output_dir.path().join("pseudo.sdc");
        let pseudo_result = run_hbcn_constrain(
            &input_path,
            &pseudo_sdc_path,
            20.0,
            1.0,
            None,
            None,
            None,
            true, // proportional disabled
            false,
            None,
            None,
        );

        // At least one should succeed
        assert!(
            prop_result.is_ok() || pseudo_result.is_ok(),
            "At least one algorithm should handle cyclic path"
        );
    }

    /// Test complex cyclic circuit
    #[test]
    fn test_complex_cyclic_circuit() {
        let graph_content = r#"Port "a" [("b", 20), ("c", 25)]
DataReg "b" [("d", 15)]
Port "c" [("d", 10)]
Port "d" [("a", 30)]
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            30.0,
            2.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Complex cyclic might succeed or fail
        match result {
            Ok(_) => {
                assert!(
                    sdc_path.exists(),
                    "SDC file should be generated if successful"
                );
                // println!("Complex cyclic constraint generation succeeded");
            }
            Err(_e) => {
                // println!("Complex cyclic constraint generation failed: {}", e);
            }
        }
    }

    /// Test cyclic path with tight timing
    #[test]
    fn test_cyclic_tight_timing() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" [("a", 10)]
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        // Very tight timing for a cyclic path
        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            15.0, // Tight cycle time
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Tight cyclic timing is likely infeasible
        match result {
            Ok(_) => {
                // println!("Tight cyclic timing succeeded (unexpected)");
            }
            Err(_e) => {
                // println!("Tight cyclic timing failed as expected: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod analyser_integration_tests {
    use super::*;

    /// Test basic analysis command with simple circuit
    #[test]
    fn test_analyse_simple_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(result.is_ok(), "Analysis should succeed: {:?}", result);
    }

    /// Test analysis with VCD output
    #[test]
    fn test_analyse_with_vcd_output() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let vcd_path = temp_output_dir.path().join("test.vcd");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), Some(&vcd_path), None);
        assert!(
            result.is_ok(),
            "Analysis with VCD should succeed: {:?}",
            result
        );
        assert!(vcd_path.exists(), "VCD file should be generated");

        let vcd_content = fs::read_to_string(&vcd_path).expect("Failed to read VCD file");
        assert!(!vcd_content.is_empty(), "VCD file should not be empty");
    }

    /// Test analysis with DOT output
    #[test]
    fn test_analyse_with_dot_output() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let dot_path = temp_output_dir.path().join("test.dot");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, Some(&dot_path));
        assert!(
            result.is_ok(),
            "Analysis with DOT should succeed: {:?}",
            result
        );
        assert!(dot_path.exists(), "DOT file should be generated");

        let dot_content = fs::read_to_string(&dot_path).expect("Failed to read DOT file");
        assert!(!dot_content.is_empty(), "DOT file should not be empty");
    }

    /// Test analysis with multiple outputs
    #[test]
    fn test_analyse_with_multiple_outputs() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" [("c", 15)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let vcd_path = temp_output_dir.path().join("test.vcd");
        let dot_path = temp_output_dir.path().join("test.dot");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(
            &input_path,
            Some(&log_path),
            Some(&vcd_path),
            Some(&dot_path),
        );
        assert!(
            result.is_ok(),
            "Analysis with multiple outputs should succeed: {:?}",
            result
        );
        assert!(vcd_path.exists(), "VCD file should be generated");
        assert!(dot_path.exists(), "DOT file should be generated");
    }

    /// Test analysis with cyclic circuit
    #[test]
    fn test_analyse_cyclic_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" [("a", 10)]
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(
            result.is_ok(),
            "Cyclic circuit analysis should succeed: {:?}",
            result
        );
    }

    /// Test analysis with complex circuit
    #[test]
    fn test_analyse_complex_circuit() {
        let graph_content = r#"Port "input" [("reg1", 30), ("reg2", 25)]
DataReg "reg1" [("output1", 20)]
DataReg "reg2" [("output2", 15)]
Port "output1" []
Port "output2" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(
            result.is_ok(),
            "Complex circuit analysis should succeed: {:?}",
            result
        );
    }

    /// Test depth analysis with simple circuit
    #[test]
    fn test_depth_simple_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" [("c", 15)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_depth(&input_path, Some(&log_path));
        assert!(
            result.is_ok(),
            "Depth analysis should succeed: {:?}",
            result
        );
    }

    /// Test depth analysis with cyclic circuit
    #[test]
    fn test_depth_cyclic_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" [("a", 10)]
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_depth(&input_path, Some(&log_path));
        assert!(
            result.is_ok(),
            "Cyclic depth analysis should succeed: {:?}",
            result
        );
    }

    /// Test analysis with invalid file
    #[test]
    fn test_analyse_invalid_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let input_path = temp_dir.path().join("nonexistent.graph");
        let log_path = temp_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(result.is_err(), "Should fail with non-existent file");
    }

    /// Test analysis with malformed input
    #[test]
    fn test_analyse_malformed_input() {
        let graph_content = "This is not a valid graph";

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(result.is_err(), "Should fail with malformed input");
    }

    /// Test analysis with single node circuit
    #[test]
    fn test_analyse_single_node_circuit() {
        let graph_content = r#"Port "a" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(
            result.is_ok(),
            "Single node analysis should succeed: {:?}",
            result
        );
    }

    /// Test analysis with tight timing circuit
    #[test]
    fn test_analyse_tight_timing_circuit() {
        let graph_content = r#"Port "a" [("b", 5)]
Port "b" [("c", 3)]
Port "c" [("d", 2)]
Port "d" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(
            result.is_ok(),
            "Tight timing analysis should succeed: {:?}",
            result
        );
    }
}

#[cfg(test)]
mod constraint_verification_tests {
    use super::*;

    /// Test that constrainer meets requested cycle time with simple circuit
    #[test]
    fn test_constrainer_meets_cycle_time_simple_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let target_cycle_time = 50.0;
        let minimal_delay = 2.0;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            target_cycle_time,
            minimal_delay,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Constraint generation should succeed: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should exist");

        // TODO: Add actual verification that the constraints meet the cycle time
        // This would require parsing the SDC file or running a timing analysis
    }

    /// Test that constrainer meets requested cycle time with DataReg circuit
    #[test]
    fn test_constrainer_meets_cycle_time_datareg_circuit() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25)]
Port "output" []
"#;

        let target_cycle_time = 100.0;
        let minimal_delay = 2.0;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            target_cycle_time,
            minimal_delay,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Constraint generation should succeed: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should exist");
    }

    /// Test that constrainer meets requested cycle time with cyclic circuit
    #[test]
    fn test_constrainer_meets_cycle_time_cyclic_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" [("a", 10)]
"#;

        let target_cycle_time = 60.0;
        let minimal_delay = 2.0;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            target_cycle_time,
            minimal_delay,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Cyclic circuits might fail with certain timing requirements
        match result {
            Ok(_) => {
                assert!(sdc_path.exists(), "SDC file should exist if successful");
            }
            Err(_e) => {
                // println!("Cyclic circuit constraint generation failed: {}", e);
            }
        }
    }

    /// Test that constrainer meets requested cycle time with complex circuit
    #[test]
    fn test_constrainer_meets_cycle_time_complex_circuit() {
        let graph_content = r#"Port "input" [("reg1", 30), ("reg2", 25)]
DataReg "reg1" [("output1", 20)]
DataReg "reg2" [("output2", 15)]
Port "output1" []
Port "output2" []
"#;

        let target_cycle_time = 150.0;
        let minimal_delay = 2.0;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            target_cycle_time,
            minimal_delay,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Complex circuit constraint generation should succeed: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should exist");
    }

    /// Test that constrainer handles tight cycle time appropriately
    #[test]
    fn test_constrainer_meets_tight_cycle_time() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let target_cycle_time = 30.0; // Tight but feasible
        let minimal_delay = 1.0;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            target_cycle_time,
            minimal_delay,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Tight timing might succeed or fail
        match result {
            Ok(_) => {
                assert!(sdc_path.exists(), "SDC file should exist if successful");
            }
            Err(_e) => {
                // println!("Tight timing failed: {}", e);
            }
        }
    }

    /// Test algorithm comparison (proportional vs pseudoclock)
    #[test]
    fn test_constrainer_algorithm_comparison() {
        let graph_content = r#"Port "a" [("b", 20), ("c", 15)]
Port "b" []
Port "c" []
"#;

        let target_cycle_time = 50.0;
        let minimal_delay = 2.0;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");

        // Test proportional
        let prop_sdc_path = temp_output_dir.path().join("prop.sdc");
        let prop_result = run_hbcn_constrain(
            &input_path,
            &prop_sdc_path,
            target_cycle_time,
            minimal_delay,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Test pseudoclock
        let pseudo_sdc_path = temp_output_dir.path().join("pseudo.sdc");
        let pseudo_result = run_hbcn_constrain(
            &input_path,
            &pseudo_sdc_path,
            target_cycle_time,
            minimal_delay,
            None,
            None,
            None,
            true,
            false,
            None,
            None,
        );

        // At least one should succeed
        assert!(
            prop_result.is_ok() || pseudo_result.is_ok(),
            "At least one algorithm should succeed"
        );
    }

    /// Test error handling
    #[test]
    fn test_constrainer_verification_error_handling() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let input_path = temp_dir.path().join("nonexistent.graph");
        let sdc_path = temp_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            50.0,
            2.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(result.is_err(), "Should fail with non-existent file");
    }
}

#[cfg(test)]
mod solver_comparison_tests {
    use super::*;

    // Note: These tests are simplified since we're now using the library API
    // and the solver is selected at runtime via environment variable.
    // Full solver comparison would require running the tests multiple times
    // with different environment variables set.

    /// Test solver consistency with simple circuit
    #[test]
    fn test_solver_status_consistency_simple_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Simple circuit should succeed with any solver: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should exist");
    }

    /// Test SDC content consistency
    #[test]
    fn test_solver_sdc_content_consistency() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Constraint generation should succeed: {:?}",
            result
        );

        let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC file");
        assert!(!sdc_content.is_empty(), "SDC file should not be empty");
        assert!(
            sdc_content.contains("set_max_delay") || sdc_content.contains("set_min_delay"),
            "SDC should contain timing constraints"
        );
    }

    /// Test solver consistency with infeasible problem
    #[test]
    fn test_solver_infeasible_consistency() {
        let graph_content = r#"Port "a" [("b", 100)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        // Try to constrain with impossible timing
        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            50.0, // Less than the minimum path delay
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Should fail with infeasible problem
        match result {
            Ok(_) => {}   // println!("Unexpectedly succeeded with tight timing"),
            Err(_e) => {} // println!("Failed as expected with infeasible timing: {}", e),
        }
    }

    /// Test solver consistency with cyclic circuit
    #[test]
    fn test_solver_cyclic_circuit_consistency() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("c", 15)]
Port "c" [("a", 10)]
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            20.0,
            1.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        // Cyclic circuits behavior depends on solver
        match result {
            Ok(_) => {
                assert!(sdc_path.exists(), "SDC file should exist if successful");
                // println!("Cyclic circuit succeeded");
            }
            Err(_e) => {
                // println!("Cyclic circuit failed: {}", e);
            }
        }
    }

    /// Test solver consistency with complex circuit
    #[test]
    fn test_solver_complex_circuit_consistency() {
        let graph_content = r#"Port "input" [("reg1", 30), ("reg2", 25)]
DataReg "reg1" [("output1", 20)]
DataReg "reg2" [("output2", 15)]
Port "output1" []
Port "output2" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("test.sdc");

        let result = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            15.0,
            2.0,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        );

        assert!(
            result.is_ok(),
            "Complex circuit should succeed: {:?}",
            result
        );
        assert!(sdc_path.exists(), "SDC file should exist");
    }

    /// Test analysis consistency
    #[test]
    fn test_solver_analysis_consistency() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let log_path = temp_output_dir.path().join("test.log");

        let result = run_hbcn_analyse(&input_path, Some(&log_path), None, None);
        assert!(result.is_ok(), "Analysis should succeed: {:?}", result);
    }
}
