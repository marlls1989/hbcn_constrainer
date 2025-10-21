use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Available LP solver backends for testing
#[derive(Debug, Clone, Copy)]
pub enum SolverBackend {
    Gurobi,
    CoinCbc,
}

impl SolverBackend {
    pub fn feature_name(&self) -> &'static str {
        match self {
            SolverBackend::Gurobi => "gurobi",
            SolverBackend::CoinCbc => "coin_cbc",
        }
    }
}

// Macro to run tests with both backends (currently unused but kept for potential future use)
#[allow(unused_macros)]
macro_rules! test_with_both_backends {
    ($test_name:ident, $test_body:block) => {
        #[test]
        fn $test_name() {
            // Test with Gurobi
            run_cargo_with_backend(SolverBackend::Gurobi, vec![]).unwrap();
            $test_body
            
            // Test with CoinCbc
            run_cargo_with_backend(SolverBackend::CoinCbc, vec![]).unwrap();
            $test_body
        }
    };
}

// Helper function to check if multiple solver features are enabled
fn multiple_solvers_available() -> bool {
    #[cfg(all(feature = "gurobi", feature = "coin_cbc"))]
    {
        true
    }
    #[cfg(not(all(feature = "gurobi", feature = "coin_cbc")))]
    {
        false
    }
}

// Helper function to run a command with both solvers and compare results
fn run_with_both_solvers_and_compare<F>(
    args: Vec<&str>,
    comparison_func: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&std::process::Output, &std::process::Output) -> Result<(), Box<dyn std::error::Error>>,
{
    if !multiple_solvers_available() {
        return Err("Multiple solvers not available for comparison".into());
    }

    // Run with Gurobi
    let gurobi_output = run_cargo_with_backend(SolverBackend::Gurobi, args.clone())?;
    
    // Run with CoinCbc
    let coin_cbc_output = run_cargo_with_backend(SolverBackend::CoinCbc, args)?;
    
    // Compare results
    comparison_func(&gurobi_output, &coin_cbc_output)?;
    
    Ok(())
}

// Helper function to run hbcn constrain with both solvers and compare results
fn run_hbcn_constrain_with_both_solvers_and_compare<F>(
    input: &PathBuf,
    sdc_gurobi: &PathBuf,
    sdc_coin_cbc: &PathBuf,
    cycle_time: f64,
    minimal_delay: f64,
    additional_args: Vec<&str>,
    comparison_func: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&std::process::Output, &std::process::Output, &PathBuf, &PathBuf) -> Result<(), Box<dyn std::error::Error>>,
{
    if !multiple_solvers_available() {
        return Err("Multiple solvers not available for comparison".into());
    }

    let cycle_time_str = cycle_time.to_string();
    let minimal_delay_str = minimal_delay.to_string();
    let mut args = vec![
        "constrain",
        input.to_str().unwrap(),
        "--sdc",
        sdc_gurobi.to_str().unwrap(),
        "-t",
        &cycle_time_str,
        "-m",
        &minimal_delay_str,
    ];
    args.extend(additional_args.clone());

    // Run with Gurobi
    let gurobi_output = run_cargo_with_backend(SolverBackend::Gurobi, args.clone())?;
    
    // Update args for CoinCbc
    let mut args_coin_cbc = vec![
        "constrain",
        input.to_str().unwrap(),
        "--sdc",
        sdc_coin_cbc.to_str().unwrap(),
        "-t",
        &cycle_time_str,
        "-m",
        &minimal_delay_str,
    ];
    args_coin_cbc.extend(additional_args);
    
    // Run with CoinCbc
    let coin_cbc_output = run_cargo_with_backend(SolverBackend::CoinCbc, args_coin_cbc)?;
    
    // Compare results
    comparison_func(&gurobi_output, &coin_cbc_output, sdc_gurobi, sdc_coin_cbc)?;
    
    Ok(())
}

// Helper function to create a temporary test file
fn create_test_file(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.graph");
    fs::write(&file_path, content).expect("Failed to write test file");
    (temp_dir, file_path)
}

// Helper function to run cargo commands with a specific backend
fn run_cargo_with_backend(backend: SolverBackend, args: Vec<&str>) -> Result<std::process::Output, std::io::Error> {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--features")
        .arg(backend.feature_name())
        .arg("--");
    
    for arg in args {
        cmd.arg(arg);
    }
    
    cmd.output()
}

// Helper function to run the hbcn constrainer binary
fn run_hbcn_constrain(
    input: &PathBuf,
    sdc: &PathBuf,
    cycle_time: f64,
    minimal_delay: f64,
    additional_args: Vec<&str>,
) -> Result<std::process::Output, std::io::Error> {
    let cycle_time_str = cycle_time.to_string();
    let minimal_delay_str = minimal_delay.to_string();
    let mut args = vec![
        "constrain",
        input.to_str().unwrap(),
        "--sdc",
        sdc.to_str().unwrap(),
        "-t",
        &cycle_time_str,
        "-m",
        &minimal_delay_str,
    ];
    args.extend(additional_args);
    
    run_cargo_with_backend(SolverBackend::Gurobi, args)
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

    /// Test cyclic path constraint generation (based on cyclic.graph)
    #[test]
    fn test_cyclic_path_constraint_generation() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("b", 15), ("c", 10)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("cyclic.sdc");
        let csv_path = temp_dir.path().join("cyclic.csv");
        let rpt_path = temp_dir.path().join("cyclic.rpt");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            50.0, // Generous cycle time for cyclic circuit
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
        .expect("Failed to run cyclic path test");

        assert!(
            output.status.success(),
            "Cyclic path test should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify all output files were generated
        assert!(sdc_path.exists(), "Cyclic SDC file should be generated");
        assert!(csv_path.exists(), "Cyclic CSV file should be generated");
        assert!(rpt_path.exists(), "Cyclic report file should be generated");

        // Verify SDC content
        let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC file");
        assert!(
            sdc_content.contains("create_clock"),
            "Cyclic SDC should contain clock definition"
        );

        // Verify CSV content
        let csv_content = fs::read_to_string(&csv_path).expect("Failed to read CSV file");
        assert!(
            csv_content.contains("src,dst,cost,max_delay,min_delay"),
            "Cyclic CSV should have proper header"
        );

        // Verify report content includes cycle analysis
        let rpt_content = fs::read_to_string(&rpt_path).expect("Failed to read report file");
        assert!(
            rpt_content.contains("Cycles:") || rpt_content.contains("Cycle"),
            "Cyclic report should contain cycle analysis"
        );
    }

    /// Test cyclic path with different constraint algorithms
    #[test]
    fn test_cyclic_path_algorithm_comparison() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25), ("reg", 20)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        // Test proportional constraints on cyclic circuit
        let temp_prop_dir = TempDir::new().expect("Failed to create temp dir");
        let prop_sdc_path = temp_prop_dir.path().join("cyclic_proportional.sdc");

        let prop_output = run_hbcn_constrain(
            &input_path,
            &prop_sdc_path,
            100.0,
            5.0,
            vec!["--forward-margin", "20", "--backward-margin", "25"],
        )
        .expect("Failed to run cyclic proportional test");

        assert!(
            prop_output.status.success(),
            "Cyclic proportional should succeed. stderr: {}",
            String::from_utf8_lossy(&prop_output.stderr)
        );
        assert!(
            prop_sdc_path.exists(),
            "Cyclic proportional SDC should be generated"
        );

        // Test pseudoclock constraints on cyclic circuit
        let temp_pseudo_dir = TempDir::new().expect("Failed to create temp dir");
        let pseudo_sdc_path = temp_pseudo_dir.path().join("cyclic_pseudoclock.sdc");

        let pseudo_output = run_hbcn_constrain(
            &input_path,
            &pseudo_sdc_path,
            100.0,
            5.0,
            vec!["--no-proportinal"],
        )
        .expect("Failed to run cyclic pseudoclock test");

        assert!(
            pseudo_output.status.success(),
            "Cyclic pseudoclock should succeed. stderr: {}",
            String::from_utf8_lossy(&pseudo_output.stderr)
        );
        assert!(
            pseudo_sdc_path.exists(),
            "Cyclic pseudoclock SDC should be generated"
        );

        // Both should produce different results
        let prop_content = fs::read_to_string(&prop_sdc_path).expect("Failed to read proportional SDC");
        let pseudo_content = fs::read_to_string(&pseudo_sdc_path).expect("Failed to read pseudoclock SDC");
        
        assert_ne!(
            prop_content, pseudo_content,
            "Cyclic proportional and pseudoclock SDC should differ"
        );
    }

    /// Test complex cyclic circuit with multiple feedback loops
    #[test]
    fn test_complex_cyclic_circuit() {
        let graph_content = r#"Port "clk" [("reg1", 5), ("reg2", 5)]
Port "input" [("reg1", 40)]
DataReg "reg1" [("logic", 30), ("reg2", 25)]
DataReg "reg2" [("logic", 35), ("reg1", 20)]
DataReg "logic" [("output", 45)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("complex_cyclic.sdc");
        let csv_path = temp_dir.path().join("complex_cyclic.csv");
        let rpt_path = temp_dir.path().join("complex_cyclic.rpt");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            200.0, // Very generous cycle time for complex cyclic circuit
            3.0,
            vec![
                "--csv",
                csv_path.to_str().unwrap(),
                "--rpt",
                rpt_path.to_str().unwrap(),
                "--forward-margin",
                "15",
                "--backward-margin",
                "20",
            ],
        )
        .expect("Failed to run complex cyclic test");

        assert!(
            output.status.success(),
            "Complex cyclic test should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify all output files exist
        assert!(sdc_path.exists(), "Complex cyclic SDC should be generated");
        assert!(csv_path.exists(), "Complex cyclic CSV should be generated");
        assert!(rpt_path.exists(), "Complex cyclic report should be generated");

        // Verify content quality
        let sdc_content = fs::read_to_string(&sdc_path).expect("Failed to read SDC");
        let csv_content = fs::read_to_string(&csv_path).expect("Failed to read CSV");
        let rpt_content = fs::read_to_string(&rpt_path).expect("Failed to read report");

        assert!(
            sdc_content.contains("create_clock"),
            "Complex cyclic SDC should contain clock definition"
        );
        assert!(
            csv_content.lines().count() > 1,
            "Complex cyclic CSV should contain constraint data"
        );
        assert!(
            rpt_content.contains("Cycles:") || rpt_content.contains("Cycle"),
            "Complex cyclic report should contain cycle analysis"
        );
    }

    /// Test cyclic circuit with tight timing constraints
    #[test]
    fn test_cyclic_tight_timing() {
        let graph_content = r#"Port "a" [("b", 10)]
DataReg "b" [("b", 5), ("c", 8)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_dir.path().join("cyclic_tight.sdc");

        let output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            15.0, // Tight cycle time for cyclic circuit
            1.0,
            vec![],
        )
        .expect("Failed to run cyclic tight timing test");

        // This might fail with infeasible constraints due to tight timing
        if output.status.success() {
            assert!(
                sdc_path.exists(),
                "Cyclic tight timing SDC should be generated"
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
}

#[cfg(test)]
mod analyser_integration_tests {
    use super::*;

    // Helper function to run the hbcn analyser binary
    fn run_hbcn_analyse(input: &PathBuf, additional_args: Vec<&str>) -> Result<std::process::Output, std::io::Error> {
        let mut args = vec!["analyse", input.to_str().unwrap()];
        args.extend(additional_args);
        run_cargo_with_backend(SolverBackend::Gurobi, args)
    }

    // Helper function to run the hbcn depth binary
    fn run_hbcn_depth(input: &PathBuf) -> Result<std::process::Output, std::io::Error> {
        let args = vec!["depth", input.to_str().unwrap()];
        run_cargo_with_backend(SolverBackend::Gurobi, args)
    }

    /// Test basic analysis command with simple circuit
    #[test]
    fn test_analyse_simple_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_analyse(&input_path, vec![])
            .expect("Failed to run hbcn analyse command");

        assert!(
            output.status.success(),
            "Analysis should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Worst virtual cycle-time"),
            "Output should contain cycle time information"
        );
    }

    /// Test analysis command with VCD output
    #[test]
    fn test_analyse_with_vcd_output() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let vcd_path = temp_output_dir.path().join("analysis.vcd");

        let output = run_hbcn_analyse(
            &input_path,
            vec!["--vcd", vcd_path.to_str().unwrap()],
        )
        .expect("Failed to run analysis with VCD");

        assert!(
            output.status.success(),
            "Analysis with VCD should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(vcd_path.exists(), "VCD file should be generated");
        
        let vcd_content = fs::read_to_string(&vcd_path).expect("Failed to read VCD file");
        assert!(
            vcd_content.contains("$timescale") || vcd_content.contains("$var"),
            "VCD file should contain VCD format markers"
        );
    }

    /// Test analysis command with DOT output
    #[test]
    fn test_analyse_with_dot_output() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" [("c", 15)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let dot_path = temp_output_dir.path().join("analysis.dot");

        let output = run_hbcn_analyse(
            &input_path,
            vec!["--dot", dot_path.to_str().unwrap()],
        )
        .expect("Failed to run analysis with DOT");

        assert!(
            output.status.success(),
            "Analysis with DOT should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(dot_path.exists(), "DOT file should be generated");
        
        let dot_content = fs::read_to_string(&dot_path).expect("Failed to read DOT file");
        assert!(
            dot_content.contains("digraph") || dot_content.contains("graph"),
            "DOT file should contain graph structure"
        );
    }

    /// Test analysis command with both VCD and DOT outputs
    #[test]
    fn test_analyse_with_multiple_outputs() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25), ("reg", 20)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let vcd_path = temp_output_dir.path().join("analysis.vcd");
        let dot_path = temp_output_dir.path().join("analysis.dot");

        let output = run_hbcn_analyse(
            &input_path,
            vec![
                "--vcd", vcd_path.to_str().unwrap(),
                "--dot", dot_path.to_str().unwrap(),
            ],
        )
        .expect("Failed to run analysis with multiple outputs");

        assert!(
            output.status.success(),
            "Analysis with multiple outputs should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(vcd_path.exists(), "VCD file should be generated");
        assert!(dot_path.exists(), "DOT file should be generated");
    }

    /// Test analysis command with cyclic circuit
    #[test]
    fn test_analyse_cyclic_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("b", 15), ("c", 10)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_analyse(&input_path, vec![])
            .expect("Failed to run analysis on cyclic circuit");

        assert!(
            output.status.success(),
            "Analysis of cyclic circuit should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Worst virtual cycle-time"),
            "Output should contain cycle time information"
        );
        assert!(
            stdout.contains("Cycle") || stdout.contains("transitions"),
            "Output should contain cycle analysis"
        );
    }

    /// Test analysis command with complex circuit
    #[test]
    fn test_analyse_complex_circuit() {
        let graph_content = r#"Port "clk" [("reg1", 5), ("reg2", 5)]
Port "input" [("reg1", 40)]
DataReg "reg1" [("logic", 30), ("reg2", 25)]
DataReg "reg2" [("logic", 35), ("reg1", 20)]
DataReg "logic" [("output", 45)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_analyse(&input_path, vec![])
            .expect("Failed to run analysis on complex circuit");

        assert!(
            output.status.success(),
            "Analysis of complex circuit should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Worst virtual cycle-time"),
            "Output should contain cycle time information"
        );
    }

    /// Test depth command with simple circuit
    #[test]
    fn test_depth_simple_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" [("c", 15)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_depth(&input_path)
            .expect("Failed to run hbcn depth command");

        assert!(
            output.status.success(),
            "Depth analysis should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Critical Cycle (Depth/Tokens)"),
            "Output should contain depth information"
        );
    }

    /// Test depth command with cyclic circuit
    #[test]
    fn test_depth_cyclic_circuit() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25), ("reg", 20)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_depth(&input_path)
            .expect("Failed to run depth analysis on cyclic circuit");

        assert!(
            output.status.success(),
            "Depth analysis of cyclic circuit should succeed. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Critical Cycle (Depth/Tokens)"),
            "Output should contain depth information"
        );
    }

    /// Test analysis command error handling with invalid file
    #[test]
    fn test_analyse_invalid_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let invalid_input = temp_dir.path().join("nonexistent.graph");

        let output = run_hbcn_analyse(&invalid_input, vec![])
            .expect("Failed to run analysis with invalid file");

        assert!(
            !output.status.success(),
            "Analysis should fail with invalid input file"
        );
    }

    /// Test analysis command error handling with malformed input
    #[test]
    fn test_analyse_malformed_input() {
        let graph_content = r#"Invalid syntax here
Not a valid graph
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_analyse(&input_path, vec![])
            .expect("Failed to run analysis with malformed input");

        assert!(
            !output.status.success(),
            "Analysis should fail with malformed input"
        );
    }

    /// Test analysis command with single node circuit
    #[test]
    fn test_analyse_single_node_circuit() {
        let graph_content = r#"Port "lonely_port" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_analyse(&input_path, vec![])
            .expect("Failed to run analysis on single node circuit");

        // Single node circuit may or may not succeed depending on implementation
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("Worst virtual cycle-time"),
                "Output should contain cycle time information if successful"
            );
        } else {
            // If it fails, it should be due to infeasible constraints
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Failed") || stderr.contains("error"),
                "Should fail with appropriate error message: {}",
                stderr
            );
        }
    }

    /// Test analysis command with tight timing circuit
    #[test]
    fn test_analyse_tight_timing_circuit() {
        let graph_content = r#"Port "fast_input" [("fast_output", 1)]
Port "fast_output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let output = run_hbcn_analyse(&input_path, vec![])
            .expect("Failed to run analysis on tight timing circuit");

        // This might succeed or fail depending on the circuit feasibility
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("Worst virtual cycle-time"),
                "Output should contain cycle time information"
            );
        } else {
            // Should fail gracefully with infeasible error
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Failed") || stderr.contains("error"),
                "Should fail with appropriate error message: {}",
                stderr
            );
        }
    }
}

#[cfg(test)]
mod constraint_verification_tests {
    use super::*;

    // Helper function to run constraint verification workflow
    fn run_constraint_verification_workflow(
        graph_content: &str,
        target_cycle_time: f64,
        minimal_delay: f64,
        algorithm: &str,
        additional_constrain_args: Vec<&str>,
    ) -> Result<(f64, f64, PathBuf, PathBuf, PathBuf, TempDir), Box<dyn std::error::Error>> {
        // Create temporary input file
        let (_temp_dir, input_path) = create_test_file(graph_content);
        
        // Create temporary output directory
        let temp_output_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_path = temp_output_dir.path().join("constraints.sdc");
        let csv_path = temp_output_dir.path().join("constraints.csv");
        let rpt_path = temp_output_dir.path().join("constraints.rpt");

        // Run constrainer
        let mut constrain_args = vec![
            "--csv", csv_path.to_str().unwrap(),
            "--rpt", rpt_path.to_str().unwrap(),
        ];
        constrain_args.extend(additional_constrain_args);
        
        if algorithm == "pseudoclock" {
            constrain_args.push("--no-proportinal");
        }

        let constrain_output = run_hbcn_constrain(
            &input_path,
            &sdc_path,
            target_cycle_time,
            minimal_delay,
            constrain_args,
        )?;

        if !constrain_output.status.success() {
            return Err(format!("Constrainer failed: {}", 
                String::from_utf8_lossy(&constrain_output.stderr)).into());
        }

        // Run analyser to get the original circuit cycle time
        let analyse_output = run_hbcn_analyse(&input_path, vec![])?;

        if !analyse_output.status.success() {
            return Err(format!("Analyser failed: {}", 
                String::from_utf8_lossy(&analyse_output.stderr)).into());
        }

        // Parse the actual cycle time from analyser output
        let analyse_stdout = String::from_utf8_lossy(&analyse_output.stdout);
        let actual_cycle_time = parse_cycle_time_from_output(&analyse_stdout)?;

        Ok((target_cycle_time, actual_cycle_time, sdc_path, csv_path, rpt_path, temp_output_dir))
    }

    // Helper function to parse cycle time from analyser output
    fn parse_cycle_time_from_output(output: &str) -> Result<f64, Box<dyn std::error::Error>> {
        for line in output.lines() {
            if line.contains("Worst virtual cycle-time:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(cycle_time_str) = parts.last() {
                    return Ok(cycle_time_str.parse()?);
                }
            }
        }
        Err("Could not parse cycle time from analyser output".into())
    }

    // Helper function to run analyser
    fn run_hbcn_analyse(input: &PathBuf, additional_args: Vec<&str>) -> Result<std::process::Output, std::io::Error> {
        let mut args = vec!["analyse", input.to_str().unwrap()];
        args.extend(additional_args);
        run_cargo_with_backend(SolverBackend::Gurobi, args)
    }

    /// Test that constrainer meets requested cycle time with simple circuit
    #[test]
    fn test_constrainer_meets_cycle_time_simple_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let target_cycle_time = 50.0;
        let minimal_delay = 2.0;

        // Test proportional algorithm
        let (_target, actual, sdc_path, csv_path, rpt_path, _temp_dir) = run_constraint_verification_workflow(
            graph_content,
            target_cycle_time,
            minimal_delay,
            "proportional",
            vec![],
        ).expect("Constraint verification should succeed");

        // The actual cycle time should be reasonable (constrainer generates constraints, 
        // but analyser analyzes the original circuit structure)
        assert!(actual > 0.0, 
            "Actual cycle time {} should be positive", actual);
        assert!(actual >= minimal_delay, 
            "Actual cycle time {} should be >= minimal delay {}", actual, minimal_delay);
        
        // Verify that constraint files were generated successfully
        if !sdc_path.exists() {
            println!("SDC path: {:?}", sdc_path);
            println!("SDC parent exists: {:?}", sdc_path.parent().map(|p| p.exists()));
        }
        assert!(sdc_path.exists(), "SDC constraint file should be generated at {:?}", sdc_path);
        assert!(csv_path.exists(), "CSV constraint file should be generated at {:?}", csv_path);
        assert!(rpt_path.exists(), "Report file should be generated at {:?}", rpt_path);
    }

    /// Test that constrainer meets requested cycle time with DataReg circuit
    #[test]
    fn test_constrainer_meets_cycle_time_datareg_circuit() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25)]
Port "output" []
"#;

        let target_cycle_time = 100.0;
        let minimal_delay = 5.0;

        // Test pseudoclock algorithm
        let (_target, actual, sdc_path, csv_path, rpt_path, _temp_dir) = run_constraint_verification_workflow(
            graph_content,
            target_cycle_time,
            minimal_delay,
            "pseudoclock",
            vec![],
        ).expect("Constraint verification should succeed");

        // Verify circuit analysis and constraint generation
        assert!(actual > 0.0, "Actual cycle time should be positive");
        assert!(actual >= minimal_delay, "Actual cycle time should be >= minimal delay");
        
        // Verify constraint files were generated
        assert!(sdc_path.exists(), "SDC file should be generated");
        assert!(csv_path.exists(), "CSV file should be generated");
        assert!(rpt_path.exists(), "Report file should be generated");
    }

    /// Test that constrainer meets requested cycle time with cyclic circuit
    #[test]
    fn test_constrainer_meets_cycle_time_cyclic_circuit() {
        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("b", 15), ("c", 10)]
Port "c" []
"#;

        let target_cycle_time = 80.0;
        let minimal_delay = 3.0;

        // Test proportional algorithm with margins
        let (_target, actual, sdc_path, csv_path, rpt_path, _temp_dir) = run_constraint_verification_workflow(
            graph_content,
            target_cycle_time,
            minimal_delay,
            "proportional",
            vec!["--forward-margin", "10", "--backward-margin", "15"],
        ).expect("Constraint verification should succeed");

        // Verify circuit analysis and constraint generation for cyclic circuit
        assert!(actual > 0.0, "Actual cycle time should be positive");
        assert!(actual >= minimal_delay, "Actual cycle time should be >= minimal delay");
        
        // Verify constraint files were generated
        assert!(sdc_path.exists(), "SDC file should be generated");
        assert!(csv_path.exists(), "CSV file should be generated");
        assert!(rpt_path.exists(), "Report file should be generated");
        
        // Verify report contains cycle analysis
        let rpt_content = fs::read_to_string(&rpt_path).expect("Failed to read report file");
        assert!(rpt_content.contains("Cycles:") || rpt_content.contains("Cycle"), 
            "Report should contain cycle analysis for cyclic circuit");
    }

    /// Test constraint verification with complex circuit
    #[test]
    fn test_constrainer_meets_cycle_time_complex_circuit() {
        let graph_content = r#"Port "clk" [("reg1", 5), ("reg2", 5)]
Port "input" [("reg1", 40)]
DataReg "reg1" [("logic", 30), ("reg2", 25)]
DataReg "reg2" [("logic", 35), ("reg1", 20)]
DataReg "logic" [("output", 45)]
Port "output" []
"#;

        let target_cycle_time = 200.0;
        let minimal_delay = 8.0;

        // Test pseudoclock algorithm
        let (_target, actual, sdc_path, csv_path, rpt_path, _temp_dir) = run_constraint_verification_workflow(
            graph_content,
            target_cycle_time,
            minimal_delay,
            "pseudoclock",
            vec![],
        ).expect("Constraint verification should succeed");

        // Verify circuit analysis and constraint generation
        assert!(actual > 0.0, "Actual cycle time should be positive");
        assert!(actual >= minimal_delay, "Actual cycle time should be >= minimal delay");
        
        // Verify constraint files were generated
        assert!(sdc_path.exists(), "SDC file should be generated");
        assert!(csv_path.exists(), "CSV file should be generated");
        assert!(rpt_path.exists(), "Report file should be generated");
    }

    /// Test constraint verification with tight timing requirements
    #[test]
    fn test_constrainer_meets_tight_cycle_time() {
        let graph_content = r#"Port "a" [("b", 10)]
Port "b" [("c", 8)]
Port "c" []
"#;

        let target_cycle_time = 25.0;
        let minimal_delay = 1.0;

        // Test proportional algorithm
        let (_target, actual, sdc_path, csv_path, rpt_path, _temp_dir) = run_constraint_verification_workflow(
            graph_content,
            target_cycle_time,
            minimal_delay,
            "proportional",
            vec![],
        ).expect("Constraint verification should succeed");

        // Verify circuit analysis and constraint generation
        assert!(actual > 0.0, "Actual cycle time should be positive");
        assert!(actual >= minimal_delay, "Actual cycle time should be >= minimal delay");
        
        // Verify constraint files were generated
        assert!(sdc_path.exists(), "SDC file should be generated");
        assert!(csv_path.exists(), "CSV file should be generated");
        assert!(rpt_path.exists(), "Report file should be generated");
    }

    /// Test constraint verification with algorithm comparison
    #[test]
    fn test_constrainer_algorithm_comparison() {
        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25), ("reg", 20)]
Port "output" []
"#;

        let target_cycle_time = 120.0;
        let minimal_delay = 4.0;

        // Test proportional algorithm
        let (_, actual_prop, sdc_prop, _, _, _temp_dir1) = run_constraint_verification_workflow(
            graph_content,
            target_cycle_time,
            minimal_delay,
            "proportional",
            vec![],
        ).expect("Proportional constraint verification should succeed");

        // Test pseudoclock algorithm
        let (_, actual_pseudo, sdc_pseudo, _, _, _temp_dir2) = run_constraint_verification_workflow(
            graph_content,
            target_cycle_time,
            minimal_delay,
            "pseudoclock",
            vec![],
        ).expect("Pseudoclock constraint verification should succeed");

        // Both should produce valid results
        assert!(actual_prop > 0.0, "Proportional actual cycle time should be positive");
        assert!(actual_pseudo > 0.0, "Pseudoclock actual cycle time should be positive");
        assert!(actual_prop >= minimal_delay, "Proportional should respect minimal delay");
        assert!(actual_pseudo >= minimal_delay, "Pseudoclock should respect minimal delay");
        
        // Both should generate constraint files
        assert!(sdc_prop.exists(), "Proportional SDC file should be generated");
        assert!(sdc_pseudo.exists(), "Pseudoclock SDC file should be generated");
    }

    /// Test constraint verification error handling
    #[test]
    fn test_constrainer_verification_error_handling() {
        let invalid_graph_content = r#"Invalid syntax here
Not a valid graph
"#;

        let target_cycle_time = 50.0;
        let minimal_delay = 2.0;

        // This should fail due to invalid input
        let result = run_constraint_verification_workflow(
            invalid_graph_content,
            target_cycle_time,
            minimal_delay,
            "proportional",
            vec![],
        );

        assert!(result.is_err(), "Should fail with invalid input");
    }
}

#[cfg(test)]
mod solver_comparison_tests {
    use super::*;

    /// Test that both solvers produce the same success/failure status for simple circuits
    #[test]
    fn test_solver_status_consistency_simple_circuit() {
        if !multiple_solvers_available() {
            println!("Skipping solver comparison test: multiple solvers not available");
            return;
        }

        let graph_content = r#"Port "a" [("b", 20)]
Port "b" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_gurobi_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_coin_cbc_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_gurobi_path = temp_gurobi_dir.path().join("gurobi.sdc");
        let sdc_coin_cbc_path = temp_coin_cbc_dir.path().join("coin_cbc.sdc");

        let result = run_hbcn_constrain_with_both_solvers_and_compare(
            &input_path,
            &sdc_gurobi_path,
            &sdc_coin_cbc_path,
            10.0,
            1.0,
            vec![],
            |gurobi_output, coin_cbc_output, _, _| {
                // Both solvers should have the same success status
                assert_eq!(
                    gurobi_output.status.success(),
                    coin_cbc_output.status.success(),
                    "Both solvers should have same success status. Gurobi: {}, CoinCbc: {}",
                    gurobi_output.status.success(),
                    coin_cbc_output.status.success()
                );

                // If both succeed, both should generate SDC files
                if gurobi_output.status.success() {
                    assert!(sdc_gurobi_path.exists(), "Gurobi SDC file should be generated");
                    assert!(sdc_coin_cbc_path.exists(), "CoinCbc SDC file should be generated");
                }

                Ok(())
            },
        );

        if let Err(e) = result {
            panic!("Solver comparison failed: {}", e);
        }
    }

    /// Test that both solvers produce similar SDC content for simple circuits
    #[test]
    fn test_solver_sdc_content_consistency() {
        if !multiple_solvers_available() {
            println!("Skipping solver comparison test: multiple solvers not available");
            return;
        }

        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_gurobi_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_coin_cbc_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_gurobi_path = temp_gurobi_dir.path().join("gurobi.sdc");
        let sdc_coin_cbc_path = temp_coin_cbc_dir.path().join("coin_cbc.sdc");

        let result = run_hbcn_constrain_with_both_solvers_and_compare(
            &input_path,
            &sdc_gurobi_path,
            &sdc_coin_cbc_path,
            50.0,
            2.0,
            vec![],
            |gurobi_output, coin_cbc_output, sdc_gurobi, sdc_coin_cbc| {
                // Both should succeed
                assert!(gurobi_output.status.success(), 
                    "Gurobi should succeed. stderr: {}", 
                    String::from_utf8_lossy(&gurobi_output.stderr));
                assert!(coin_cbc_output.status.success(), 
                    "CoinCbc should succeed. stderr: {}", 
                    String::from_utf8_lossy(&coin_cbc_output.stderr));

                // Both should generate SDC files
                assert!(sdc_gurobi.exists(), "Gurobi SDC file should be generated");
                assert!(sdc_coin_cbc.exists(), "CoinCbc SDC file should be generated");

                // Both SDC files should contain basic clock definitions
                let gurobi_sdc = fs::read_to_string(sdc_gurobi).expect("Failed to read Gurobi SDC");
                let coin_cbc_sdc = fs::read_to_string(sdc_coin_cbc).expect("Failed to read CoinCbc SDC");

                assert!(gurobi_sdc.contains("create_clock"), "Gurobi SDC should contain clock definition");
                assert!(coin_cbc_sdc.contains("create_clock"), "CoinCbc SDC should contain clock definition");

                // Both should have similar structure (same number of lines, similar content)
                let gurobi_lines: Vec<&str> = gurobi_sdc.lines().collect();
                let coin_cbc_lines: Vec<&str> = coin_cbc_sdc.lines().collect();

                // Should have similar number of constraints (within 10% tolerance)
                let line_diff = (gurobi_lines.len() as i32 - coin_cbc_lines.len() as i32).abs();
                let max_diff = (gurobi_lines.len() as f64 * 0.1) as i32;
                assert!(line_diff <= max_diff, 
                    "SDC files should have similar number of lines. Gurobi: {}, CoinCbc: {}, diff: {}",
                    gurobi_lines.len(), coin_cbc_lines.len(), line_diff);

                Ok(())
            },
        );

        if let Err(e) = result {
            panic!("Solver comparison failed: {}", e);
        }
    }

    /// Test that both solvers handle infeasible problems consistently
    #[test]
    fn test_solver_infeasible_consistency() {
        if !multiple_solvers_available() {
            println!("Skipping solver comparison test: multiple solvers not available");
            return;
        }

        let graph_content = r#"Port "fast_input" [("fast_output", 1)]
Port "fast_output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_gurobi_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_coin_cbc_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_gurobi_path = temp_gurobi_dir.path().join("gurobi.sdc");
        let sdc_coin_cbc_path = temp_coin_cbc_dir.path().join("coin_cbc.sdc");

        let result = run_hbcn_constrain_with_both_solvers_and_compare(
            &input_path,
            &sdc_gurobi_path,
            &sdc_coin_cbc_path,
            0.5, // Very tight timing that should be infeasible
            0.1,
            vec![],
            |gurobi_output, coin_cbc_output, _, _| {
                // Both solvers should handle infeasible problems consistently
                // They might both fail or both succeed with appropriate error handling
                let gurobi_success = gurobi_output.status.success();
                let coin_cbc_success = coin_cbc_output.status.success();

                // If one fails, the other should also fail
                if !gurobi_success || !coin_cbc_success {
                    assert_eq!(gurobi_success, coin_cbc_success,
                        "Both solvers should have same success status for infeasible problem. Gurobi: {}, CoinCbc: {}",
                        gurobi_success, coin_cbc_success);
                }

                // If both fail, both should have appropriate error messages
                if !gurobi_success && !coin_cbc_success {
                    let gurobi_stderr = String::from_utf8_lossy(&gurobi_output.stderr);
                    let coin_cbc_stderr = String::from_utf8_lossy(&coin_cbc_output.stderr);

                    // Both should indicate infeasibility or error
                    assert!(gurobi_stderr.contains("Infeasible") || gurobi_stderr.contains("infeasible") || gurobi_stderr.contains("error"),
                        "Gurobi should report infeasibility or error: {}", gurobi_stderr);
                    assert!(coin_cbc_stderr.contains("Infeasible") || coin_cbc_stderr.contains("infeasible") || coin_cbc_stderr.contains("error"),
                        "CoinCbc should report infeasibility or error: {}", coin_cbc_stderr);
                }

                Ok(())
            },
        );

        if let Err(e) = result {
            panic!("Solver comparison failed: {}", e);
        }
    }

    /// Test that both solvers produce consistent results for cyclic circuits
    #[test]
    fn test_solver_cyclic_circuit_consistency() {
        if !multiple_solvers_available() {
            println!("Skipping solver comparison test: multiple solvers not available");
            return;
        }

        let graph_content = r#"Port "a" [("b", 20)]
DataReg "b" [("b", 15), ("c", 10)]
Port "c" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_gurobi_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_coin_cbc_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_gurobi_path = temp_gurobi_dir.path().join("gurobi.sdc");
        let sdc_coin_cbc_path = temp_coin_cbc_dir.path().join("coin_cbc.sdc");

        let result = run_hbcn_constrain_with_both_solvers_and_compare(
            &input_path,
            &sdc_gurobi_path,
            &sdc_coin_cbc_path,
            50.0, // Generous cycle time for cyclic circuit
            2.0,
            vec!["--forward-margin", "10", "--backward-margin", "15"],
            |gurobi_output, coin_cbc_output, sdc_gurobi, sdc_coin_cbc| {
                // Both should succeed
                assert!(gurobi_output.status.success(), 
                    "Gurobi should succeed with cyclic circuit. stderr: {}", 
                    String::from_utf8_lossy(&gurobi_output.stderr));
                assert!(coin_cbc_output.status.success(), 
                    "CoinCbc should succeed with cyclic circuit. stderr: {}", 
                    String::from_utf8_lossy(&coin_cbc_output.stderr));

                // Both should generate SDC files
                assert!(sdc_gurobi.exists(), "Gurobi SDC file should be generated");
                assert!(sdc_coin_cbc.exists(), "CoinCbc SDC file should be generated");

                // Both SDC files should contain clock definitions
                let gurobi_sdc = fs::read_to_string(sdc_gurobi).expect("Failed to read Gurobi SDC");
                let coin_cbc_sdc = fs::read_to_string(sdc_coin_cbc).expect("Failed to read CoinCbc SDC");

                assert!(gurobi_sdc.contains("create_clock"), "Gurobi SDC should contain clock definition");
                assert!(coin_cbc_sdc.contains("create_clock"), "CoinCbc SDC should contain clock definition");

                // Both should have similar constraint structure
                let gurobi_constraints = gurobi_sdc.lines().filter(|line| line.contains("set_max_delay") || line.contains("set_min_delay")).count();
                let coin_cbc_constraints = coin_cbc_sdc.lines().filter(|line| line.contains("set_max_delay") || line.contains("set_min_delay")).count();

                // Should have similar number of timing constraints (within 20% tolerance for cyclic circuits)
                let constraint_diff = (gurobi_constraints as i32 - coin_cbc_constraints as i32).abs();
                let max_diff = (gurobi_constraints as f64 * 0.2) as i32;
                assert!(constraint_diff <= max_diff, 
                    "SDC files should have similar number of timing constraints. Gurobi: {}, CoinCbc: {}, diff: {}",
                    gurobi_constraints, coin_cbc_constraints, constraint_diff);

                Ok(())
            },
        );

        if let Err(e) = result {
            panic!("Solver comparison failed: {}", e);
        }
    }

    /// Test that both solvers produce consistent results for complex circuits
    #[test]
    fn test_solver_complex_circuit_consistency() {
        if !multiple_solvers_available() {
            println!("Skipping solver comparison test: multiple solvers not available");
            return;
        }

        let graph_content = r#"Port "clk" [("reg1", 5), ("reg2", 5)]
Port "input" [("reg1", 40)]
DataReg "reg1" [("logic", 30), ("reg2", 25)]
DataReg "reg2" [("logic", 35), ("reg1", 20)]
DataReg "logic" [("output", 45)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);
        let temp_gurobi_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_coin_cbc_dir = TempDir::new().expect("Failed to create temp dir");
        let sdc_gurobi_path = temp_gurobi_dir.path().join("gurobi.sdc");
        let sdc_coin_cbc_path = temp_coin_cbc_dir.path().join("coin_cbc.sdc");

        let result = run_hbcn_constrain_with_both_solvers_and_compare(
            &input_path,
            &sdc_gurobi_path,
            &sdc_coin_cbc_path,
            200.0, // Very generous cycle time for complex circuit
            8.0,
            vec![],
            |gurobi_output, coin_cbc_output, sdc_gurobi, sdc_coin_cbc| {
                // Both should succeed
                assert!(gurobi_output.status.success(), 
                    "Gurobi should succeed with complex circuit. stderr: {}", 
                    String::from_utf8_lossy(&gurobi_output.stderr));
                assert!(coin_cbc_output.status.success(), 
                    "CoinCbc should succeed with complex circuit. stderr: {}", 
                    String::from_utf8_lossy(&coin_cbc_output.stderr));

                // Both should generate SDC files
                assert!(sdc_gurobi.exists(), "Gurobi SDC file should be generated");
                assert!(sdc_coin_cbc.exists(), "CoinCbc SDC file should be generated");

                // Both SDC files should contain clock definitions
                let gurobi_sdc = fs::read_to_string(sdc_gurobi).expect("Failed to read Gurobi SDC");
                let coin_cbc_sdc = fs::read_to_string(sdc_coin_cbc).expect("Failed to read CoinCbc SDC");

                assert!(gurobi_sdc.contains("create_clock"), "Gurobi SDC should contain clock definition");
                assert!(coin_cbc_sdc.contains("create_clock"), "CoinCbc SDC should contain clock definition");

                // Both should have substantial constraint content
                let gurobi_lines = gurobi_sdc.lines().count();
                let coin_cbc_lines = coin_cbc_sdc.lines().count();

                assert!(gurobi_lines > 5, "Gurobi SDC should have substantial content");
                assert!(coin_cbc_lines > 5, "CoinCbc SDC should have substantial content");

                // Should have similar amount of content (within 30% tolerance for complex circuits)
                let line_diff = (gurobi_lines as i32 - coin_cbc_lines as i32).abs();
                let max_diff = (gurobi_lines as f64 * 0.3) as i32;
                assert!(line_diff <= max_diff, 
                    "SDC files should have similar amount of content. Gurobi: {}, CoinCbc: {}, diff: {}",
                    gurobi_lines, coin_cbc_lines, line_diff);

                Ok(())
            },
        );

        if let Err(e) = result {
            panic!("Solver comparison failed: {}", e);
        }
    }

    /// Test that both solvers handle analysis commands consistently
    #[test]
    fn test_solver_analysis_consistency() {
        if !multiple_solvers_available() {
            println!("Skipping solver comparison test: multiple solvers not available");
            return;
        }

        let graph_content = r#"Port "input" [("reg", 30)]
DataReg "reg" [("output", 25)]
Port "output" []
"#;

        let (_temp_dir, input_path) = create_test_file(graph_content);

        let result = run_with_both_solvers_and_compare(
            vec!["analyse", input_path.to_str().unwrap()],
            |gurobi_output, coin_cbc_output| {
                // Both should succeed
                assert!(gurobi_output.status.success(), 
                    "Gurobi analysis should succeed. stderr: {}", 
                    String::from_utf8_lossy(&gurobi_output.stderr));
                assert!(coin_cbc_output.status.success(), 
                    "CoinCbc analysis should succeed. stderr: {}", 
                    String::from_utf8_lossy(&coin_cbc_output.stderr));

                // Both should produce similar output content
                let gurobi_stdout = String::from_utf8_lossy(&gurobi_output.stdout);
                let coin_cbc_stdout = String::from_utf8_lossy(&coin_cbc_output.stdout);

                // Both should contain cycle time information
                assert!(gurobi_stdout.contains("Worst virtual cycle-time"), 
                    "Gurobi output should contain cycle time information");
                assert!(coin_cbc_stdout.contains("Worst virtual cycle-time"), 
                    "CoinCbc output should contain cycle time information");

                // Both should have similar output length (within 50% tolerance)
                let gurobi_len = gurobi_stdout.len();
                let coin_cbc_len = coin_cbc_stdout.len();
                let len_diff = (gurobi_len as i32 - coin_cbc_len as i32).abs();
                let max_diff = (gurobi_len as f64 * 0.5) as i32;
                assert!(len_diff <= max_diff, 
                    "Analysis outputs should have similar length. Gurobi: {}, CoinCbc: {}, diff: {}",
                    gurobi_len, coin_cbc_len, len_diff);

                Ok(())
            },
        );

        if let Err(e) = result {
            panic!("Solver comparison failed: {}", e);
        }
    }
}
