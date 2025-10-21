use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Helper function to create a temporary test file
fn create_test_file(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.graph");
    fs::write(&file_path, content).expect("Failed to write test file");
    (temp_dir, file_path)
}

// Helper function to run the hbcn constrainer binary
fn run_hbcn_constrain(
    input: &PathBuf,
    sdc: &PathBuf,
    cycle_time: f64,
    minimal_delay: f64,
    additional_args: Vec<&str>,
) -> Result<std::process::Output, std::io::Error> {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--")
        .arg("constrain")
        .arg(input)
        .arg("--sdc")
        .arg(sdc)
        .arg("-t")
        .arg(cycle_time.to_string())
        .arg("-m")
        .arg(minimal_delay.to_string());

    for arg in additional_args {
        cmd.arg(arg);
    }

    cmd.output()
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

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            vec![
                "--csv",
                csv_path.to_str().unwrap(),
                "--rpt",
                rpt_path.to_str().unwrap(),
            ],
        )
        .expect("Failed to run hbcn constrain command");

        assert!(
            output.status.success(),
            "Command should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify all output files were generated correctly
        assert!(sdc_path.exists(), "SDC file should be generated");
        assert!(csv_path.exists(), "CSV file should be generated");
        assert!(rpt_path.exists(), "Report file should be generated");

        let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC file");
        assert!(
            sdc_content.contains("create_clock"),
            "SDC should contain clock definition"
        );

        let csv_content = fs::read_to_string(&csv_path).expect("Failed to read CSV file");
        assert!(
            csv_content.contains("src,dst,cost,max_delay,min_delay"),
            "CSV should have header"
        );

        let rpt_content = fs::read_to_string(&rpt_path).expect("Failed to read report file");
        assert!(
            rpt_content.contains("Cycle time constraint"),
            "Report should contain cycle time constraint"
        );
    }

    /// Test proportional vs pseudoclock constraint modes
    #[test]
    fn test_proportional_vs_pseudoclock_constraints() {
        let graph_content = r#"Port "input" [("reg", 50)]
DataReg "reg" [("output", 75)]  
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        // Test proportional constraints
        let temp_prop_dir = TempDir::new().expect("Failed to create temp dir");
        let prop_sdc_path = temp_prop_dir.path().join("proportional.sdc");

        let prop_output = run_hbcn_constrain(
            &input_path,
            &prop_sdc_path,
            15.0,
            2.0,
            vec!["--forward-margin", "10", "--backward-margin", "15"],
        )
        .expect("Failed to run proportional constraints");

        assert!(
            prop_output.status.success(),
            "Proportional constraints should succeed. stderr: {}",
            String::from_utf8_lossy(&prop_output.stderr)
        );
        assert!(
            prop_sdc_path.exists(),
            "Proportional SDC file should be generated"
        );

        // Test pseudoclock constraints
        let temp_pseudo_dir = TempDir::new().expect("Failed to create temp dir");
        let pseudo_sdc_path = temp_pseudo_dir.path().join("pseudoclock.sdc");

        let pseudo_output = run_hbcn_constrain(
            &input_path,
            &pseudo_sdc_path,
            15.0,
            2.0,
            vec!["--no-proportinal"],
        )
        .expect("Failed to run pseudoclock constraints");

        assert!(
            pseudo_output.status.success(),
            "Pseudoclock constraints should succeed. stderr: {}",
            String::from_utf8_lossy(&pseudo_output.stderr)
        );
        assert!(
            pseudo_sdc_path.exists(),
            "Pseudoclock SDC file should be generated"
        );

        // Compare the outputs - they should be different
        let prop_sdc_content =
            fs::read_to_string(&prop_sdc_path).expect("Failed to read proportional SDC");
        let pseudo_sdc_content =
            fs::read_to_string(&pseudo_sdc_path).expect("Failed to read pseudoclock SDC");

        assert!(
            prop_sdc_content.contains("create_clock"),
            "Proportional SDC should contain clock"
        );
        assert!(
            pseudo_sdc_content.contains("create_clock"),
            "Pseudoclock SDC should contain clock"
        );
        assert_ne!(
            prop_sdc_content, pseudo_sdc_content,
            "Proportional and pseudoclock SDC should differ"
        );
    }

    /// Test forward completion enable/disable effects
    #[test]
    fn test_forward_completion_effects() {
        let graph_content = r#"Port "a" [("reg1", 30)]
DataReg "reg1" [("reg2", 40)]
DataReg "reg2" [("b", 50)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        // Test with forward completion enabled (default)
        let temp_fc_dir = TempDir::new().expect("Failed to create temp dir");
        let fc_sdc_path = temp_fc_dir.path().join("forward_completion.sdc");
        let fc_csv_path = temp_fc_dir.path().join("forward_completion.csv");

        let fc_output = run_hbcn_constrain(
            &input_path,
            &fc_sdc_path,
            20.0,
            1.5,
            vec!["--csv", fc_csv_path.to_str().unwrap()],
        )
        .expect("Failed to run with forward completion");

        assert!(
            fc_output.status.success(),
            "Forward completion should succeed. stderr: {}",
            String::from_utf8_lossy(&fc_output.stderr)
        );

        // Test with forward completion disabled
        let temp_no_fc_dir = TempDir::new().expect("Failed to create temp dir");
        let no_fc_sdc_path = temp_no_fc_dir.path().join("no_forward_completion.sdc");
        let no_fc_csv_path = temp_no_fc_dir.path().join("no_forward_completion.csv");

        let no_fc_output = run_hbcn_constrain(
            &input_path,
            &no_fc_sdc_path,
            20.0,
            1.5,
            vec![
                "--csv",
                no_fc_csv_path.to_str().unwrap(),
                "--no-forward-completion",
            ],
        )
        .expect("Failed to run without forward completion");

        assert!(
            no_fc_output.status.success(),
            "No forward completion should succeed. stderr: {}",
            String::from_utf8_lossy(&no_fc_output.stderr)
        );

        // Verify both files exist
        assert!(
            fc_sdc_path.exists() && fc_csv_path.exists(),
            "Forward completion files should exist"
        );
        assert!(
            no_fc_sdc_path.exists() && no_fc_csv_path.exists(),
            "No forward completion files should exist"
        );
    }

    /// Test margin parameter functionality
    #[test]
    fn test_margin_parameters() {
        let graph_content = r#"Port "input" [("process", 25)]
DataReg "process" [("output", 35)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("margin_test.sdc");
        let csv_path = temp_dir.path().join("margin_test.csv");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            12.0,
            1.0,
            vec![
                "--csv",
                csv_path.to_str().unwrap(),
                "--forward-margin",
                "20",
                "--backward-margin",
                "25",
            ],
        )
        .expect("Failed to run with margins");

        assert!(
            output.status.success(),
            "Margin test should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            sdc_path.exists(),
            "SDC file should be generated with margins"
        );
        assert!(
            csv_path.exists(),
            "CSV file should be generated with margins"
        );

        let csv_content = fs::read_to_string(&csv_path).expect("Failed to read CSV");
        assert!(
            csv_content.contains("max_delay"),
            "CSV should contain timing constraints"
        );
    }

    /// Test VCD output generation
    #[test]
    fn test_vcd_output_generation() {
        let graph_content = r#"Port "a" [("b", 15), ("c", 25)]
Port "b" [("d", 10)]
Port "c" [("d", 20)]
Port "d" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("vcd_test.sdc");
        let vcd_path = temp_dir.path().join("timing.vcd");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            8.0,
            0.5,
            vec!["--vcd", vcd_path.to_str().unwrap()],
        )
        .expect("Failed to run VCD generation");

        assert!(
            output.status.success(),
            "VCD generation should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(vcd_path.exists(), "VCD file should be generated");
        let vcd_content = fs::read_to_string(&vcd_path).expect("Failed to read VCD file");
        assert!(
            vcd_content.contains("$dumpvars") || vcd_content.contains("$var"),
            "VCD file should contain VCD format markers"
        );
    }

    /// Test complex circuit with multiple paths and registers
    #[test]
    fn test_complex_circuit_constraints() {
        let graph_content = r#"Port "clk" [("reg1", 5), ("reg2", 5), ("reg3", 5)]
Port "input_a" [("reg1", 45)]
Port "input_b" [("reg2", 55)]
DataReg "reg1" [("logic", 30)]
DataReg "reg2" [("logic", 25)]
DataReg "reg3" [("output", 40)]
DataReg "logic" [("reg3", 35)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("complex.sdc");
        let csv_path = temp_dir.path().join("complex.csv");
        let rpt_path = temp_dir.path().join("complex.rpt");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            25.0,
            2.0,
            vec![
                "--csv",
                csv_path.to_str().unwrap(),
                "--rpt",
                rpt_path.to_str().unwrap(),
                "--forward-margin",
                "10",
                "--backward-margin",
                "15",
            ],
        )
        .expect("Failed to run complex circuit");

        assert!(
            output.status.success(),
            "Complex circuit should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify all output files exist
        assert!(sdc_path.exists(), "Complex circuit SDC should be generated");
        assert!(csv_path.exists(), "Complex circuit CSV should be generated");
        assert!(
            rpt_path.exists(),
            "Complex circuit report should be generated"
        );

        // Verify content quality
        let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC");
        let csv_content = fs::read_to_string(&csv_path).expect("Failed to read CSV");
        let rpt_content = fs::read_to_string(&rpt_path).expect("Failed to read report");

        assert!(
            sdc_content.contains("create_clock"),
            "SDC should contain clock definition"
        );
        assert!(
            csv_content.lines().count() > 1,
            "CSV should contain constraint data"
        );
        assert!(
            rpt_content.contains("Cycles:") || rpt_content.contains("Cycle"),
            "Report should contain cycle analysis"
        );
    }

    /// Test edge case with very tight timing constraints
    #[test]
    fn test_edge_case_very_tight_timing() {
        let graph_content = r#"Port "fast_input" [("fast_output", 1)]
Port "fast_output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("tight_timing.sdc");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            0.5, // Very tight timing
            0.1,
            vec![],
        )
        .expect("Failed to run tight timing test");

        // This might fail with infeasible constraints, which is expected behaviour
        if output.status.success() {
            assert!(
                sdc_path.exists(),
                "SDC should be generated for tight timing"
            );
            let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC");
            assert!(
                sdc_content.contains("create_clock"),
                "Should contain clock definition"
            );
        } else {
            // Should fail gracefully with infeasible error
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Infeasible") || stderr.contains("infeasible"),
                "Should fail with infeasible error for impossible timing: {}",
                stderr
            );
        }
    }

    /// Test zero minimal delay parameter
    #[test]
    fn test_zero_minimal_delay() {
        let graph_content = r#"Port "src" [("dst", 10)]
Port "dst" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("zero_delay.sdc");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            5.0,
            0.0, // Zero minimal delay
            vec![],
        )
        .expect("Failed to run zero delay test");

        assert!(
            output.status.success(),
            "Zero minimal delay should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            sdc_path.exists(),
            "SDC should be generated with zero minimal delay"
        );
    }

    /// Test boundary margin values (0% and 99%)
    #[test]
    fn test_boundary_margin_values() {
        let graph_content = r#"Port "input" [("output", 30)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        // Test minimum margins (0%) - these should be more likely to succeed
        let temp_min_dir = TempDir::new().expect("Failed to create temp dir");
        let min_sdc_path = temp_min_dir.path().join("min_margin.sdc");

        let min_output = run_hbcn_constrain(
            &input_path,
            &min_sdc_path,
            20.0,
            1.0,
            vec!["--forward-margin", "0", "--backward-margin", "0"],
        )
        .expect("Failed to run min margin test");

        assert!(
            min_output.status.success(),
            "Min margins should succeed. stderr: {}",
            String::from_utf8_lossy(&min_output.stderr)
        );
        assert!(
            min_sdc_path.exists(),
            "SDC should be generated with minimum margins"
        );

        // Test maximum margins (99%) - these may be infeasible
        let temp_max_dir = TempDir::new().expect("Failed to create temp dir");
        let max_sdc_path = temp_max_dir.path().join("max_margin.sdc");

        let max_output = run_hbcn_constrain(
            &input_path,
            &max_sdc_path,
            20.0,
            1.0,
            vec!["--forward-margin", "99", "--backward-margin", "99"],
        )
        .expect("Failed to run max margin test");

        // Maximum margins might be infeasible, which is acceptable
        if max_output.status.success() {
            assert!(
                max_sdc_path.exists(),
                "SDC should be generated with maximum margins"
            );
        } else {
            let stderr = String::from_utf8_lossy(&max_output.stderr);
            assert!(
                stderr.contains("Infeasible") || stderr.contains("infeasible"),
                "Max margin failure should be due to infeasible constraints: {}",
                stderr
            );
        }
    }

    /// Test single node circuit (edge case)
    #[test]
    fn test_single_node_circuit() {
        let graph_content = r#"Port "lonely_port" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("single_node.sdc");
        let csv_path = temp_dir.path().join("single_node.csv");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            10.0,
            1.0,
            vec!["--csv", csv_path.to_str().unwrap()],
        )
        .expect("Failed to run single node test");

        // Single node with no connections may be infeasible, which is expected
        if output.status.success() {
            // If it succeeds, verify files are generated correctly
            assert!(sdc_path.exists(), "SDC should be generated for single node");
            assert!(csv_path.exists(), "CSV should be generated for single node");

            let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC");
            assert!(
                sdc_content.contains("create_clock"),
                "SDC should still contain clock definition"
            );

            let csv_content = fs::read_to_string(&csv_path).expect("Failed to read CSV");
            assert!(
                csv_content.contains("src,dst,cost,max_delay,min_delay"),
                "CSV should have header even with no data"
            );
        } else {
            // If it fails, it should be due to infeasible constraints
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Infeasible") || stderr.contains("infeasible"),
                "Single node failure should be due to infeasible constraints: {}",
                stderr
            );
        }
    }

    /// Test error handling with invalid input file
    #[test]
    fn test_invalid_input_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let invalid_input = temp_dir.path().join("nonexistent.graph");
        let sdc_path = temp_dir.path().join("output.sdc");

        let output = run_hbcn_constrain(&invalid_input, &sdc_path, 10.0, 1.0, vec![])
            .expect("Failed to run invalid input test");

        // Should fail gracefully
        assert!(
            !output.status.success(),
            "Should fail with invalid input file"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("No such file")
                || stderr.contains("not found")
                || stderr.contains("error"),
            "Should report file not found error: {}",
            stderr
        );
    }

    /// Test error handling with malformed graph input
    #[test]
    fn test_malformed_graph_input() {
        let graph_content = r#"Invalid syntax here
Not a valid graph
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("malformed.sdc");

        let output = run_hbcn_constrain(&input_path, &sdc_path, 10.0, 1.0, vec![])
            .expect("Failed to run malformed input test");

        // Should fail gracefully with parsing error
        assert!(!output.status.success(), "Should fail with malformed input");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Invalid token")
                || stderr.contains("parse")
                || stderr.contains("error"),
            "Should report parsing error: {}",
            stderr
        );
    }
}
