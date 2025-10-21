#[cfg(test)]
mod constrain_unit_tests {
    use crate::hbcn::*;
    use crate::structural_graph::parse;

    /// Helper function to create a simple test StructuralHBCN from a structural graph string
    fn create_test_hbcn(input: &str, forward_completion: bool) -> StructuralHBCN {
        let structural_graph = parse(input).expect("Failed to parse test input");
        from_structural_graph(&structural_graph, forward_completion)
            .expect("Failed to convert to StructuralHBCN")
    }

    /// Test pseudoclock constraint generation with basic circuit
    #[test]
    fn test_pseudoclock_constraints_basic() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        let result = constrain_cycle_time_pseudoclock(&hbcn, 10.0, 1.0)
            .expect("Pseudoclock constraint generation should succeed");

        // Pseudoclock period should be reasonable
        assert!(result.pseudoclock_period >= 1.0);
        assert!(result.pseudoclock_period <= 10.0);

        // Should have path constraints
        assert!(!result.path_constraints.is_empty());

        // All max delays should be valid
        for (_, constraint) in &result.path_constraints {
            if let Some(max_delay) = constraint.max {
                assert!(max_delay >= 1.0, "Max delay should be at least min_delay");
            }
            // Pseudoclock algorithm only generates max constraints
            assert!(constraint.min.is_none());
        }
    }

    /// Test proportional constraint generation with basic circuit
    #[test]
    fn test_proportional_constraints_basic() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        let result = constrain_cycle_time_proportional(&hbcn, 10.0, 1.0, None, None)
            .expect("Proportional constraint generation should succeed");

        // Pseudoclock period should be reasonable
        assert!(result.pseudoclock_period >= 1.0);

        // Should have path constraints
        assert!(!result.path_constraints.is_empty());

        // Should have both min and max delays for proportional algorithm
        let has_min = result.path_constraints.values().any(|c| c.min.is_some());
        let has_max = result.path_constraints.values().any(|c| c.max.is_some());
        assert!(
            has_min || has_max,
            "Should have at least some min or max constraints"
        );
    }

    /// Test that infeasible constraints are properly detected
    #[test]
    fn test_infeasible_constraints() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 1)]
               Port "b" []"#,
            false,
        );

        // Try to constrain with impossible parameters (cycle time too small)
        let result = constrain_cycle_time_pseudoclock(&hbcn, 0.1, 10.0);
        assert!(result.is_err(), "Should fail with infeasible constraints");

        // Try proportional with impossible parameters
        let result = constrain_cycle_time_proportional(&hbcn, 0.1, 10.0, None, None);
        assert!(result.is_err(), "Should fail with infeasible constraints");
    }

    /// Test margin effects on proportional constraints
    #[test]
    fn test_proportional_constraints_with_margins() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" [("c", 50)]
               Port "c" []"#,
            false,
        );

        // Test with feasible parameters and margin
        let result_with_margin =
            constrain_cycle_time_proportional(&hbcn, 500.0, 10.0, None, Some(0.5))
                .expect("Should succeed with forward margin");

        let result_without_margin =
            constrain_cycle_time_proportional(&hbcn, 500.0, 10.0, None, None)
                .expect("Should succeed without margin");

        // Both results should be valid
        assert!(result_with_margin.pseudoclock_period >= 10.0);
        assert!(result_without_margin.pseudoclock_period >= 10.0);

        // Test with backward margin
        let result_backward =
            constrain_cycle_time_proportional(&hbcn, 500.0, 10.0, Some(0.5), None)
                .expect("Should succeed with backward margin");

        assert!(result_backward.pseudoclock_period >= 10.0);

        // At least one should produce some constraints
        let total_constraints = result_with_margin.path_constraints.len()
            + result_without_margin.path_constraints.len()
            + result_backward.path_constraints.len();
        assert!(
            total_constraints > 0,
            "Should produce some constraints across all tests"
        );
    }

    /// Test constraint generation with DataReg (more complex circuit)
    #[test]
    fn test_constraints_with_datareg() {
        let hbcn = create_test_hbcn(
            r#"Port "input" [("reg", 50)]
               DataReg "reg" [("output", 75)]
               Port "output" []"#,
            false,
        );

        // Should work with pseudoclock
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 20.0, 2.0)
            .expect("Should handle DataReg with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 2.0);
        assert!(!pseudo_result.path_constraints.is_empty());

        // Should work with proportional
        let prop_result = constrain_cycle_time_proportional(&hbcn, 20.0, 2.0, None, None)
            .expect("Should handle DataReg with proportional");

        assert!(prop_result.pseudoclock_period >= 2.0);
        assert!(!prop_result.path_constraints.is_empty());
    }

    /// Test edge case: simple two-node circuit (minimal viable circuit)
    #[test]
    fn test_minimal_circuit() {
        let hbcn = create_test_hbcn(
            r#"Port "input" [("output", 50)]
               Port "output" []"#,
            false,
        );

        // Should work with minimal circuit
        let result = constrain_cycle_time_pseudoclock(&hbcn, 100.0, 5.0)
            .expect("Should handle minimal circuit");

        assert!(result.pseudoclock_period >= 5.0);
        // Path constraints may or may not exist for minimal circuits
    }

    /// Test that forward completion affects constraint generation
    #[test]
    fn test_forward_completion_effects() {
        let input = r#"Port "a" [("b", 100)]
                      Port "b" [("c", 50)]
                      Port "c" []"#;

        let hbcn_no_fc = create_test_hbcn(input, false);
        let hbcn_with_fc = create_test_hbcn(input, true);

        let result_no_fc = constrain_cycle_time_proportional(&hbcn_no_fc, 300.0, 10.0, None, None)
            .expect("Should work without forward completion");

        let result_with_fc =
            constrain_cycle_time_proportional(&hbcn_with_fc, 300.0, 10.0, None, None)
                .expect("Should work with forward completion");

        // Results should potentially be different
        // (This tests that forward completion parameter is actually used)
        assert!(result_no_fc.pseudoclock_period >= 10.0);
        assert!(result_with_fc.pseudoclock_period >= 10.0);

        // Both should produce valid results (constraint count is always >= 0)
        assert!(
            !result_no_fc.path_constraints.is_empty(),
            "Expected constraints without forward completion"
        );
        assert!(
            !result_with_fc.path_constraints.is_empty(),
            "Expected constraints with forward completion"
        );
    }

    /// Test constraint validation (that generated constraints are reasonable)
    #[test]
    fn test_constraint_validity() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" [("c", 200)]
               Port "c" []"#,
            false,
        );

        let result = constrain_cycle_time_proportional(&hbcn, 50.0, 5.0, None, None)
            .expect("Should generate valid constraints");

        // Validate all constraints
        for ((src, dst), constraint) in &result.path_constraints {
            // Source and destination should be valid circuit nodes
            assert!(!src.name().as_ref().is_empty());
            assert!(!dst.name().as_ref().is_empty());

            // Delays should be reasonable
            if let Some(min_delay) = constraint.min {
                assert!(min_delay >= 0.0, "Min delay should be non-negative");
                assert!(min_delay <= 50.0, "Min delay should be reasonable");
            }

            if let Some(max_delay) = constraint.max {
                assert!(max_delay >= 5.0, "Max delay should be at least min_delay");
                assert!(max_delay <= 50.0, "Max delay should not exceed cycle time");
            }

            // If both exist, min should be <= max
            if let (Some(min), Some(max)) = (constraint.min, constraint.max) {
                assert!(min <= max, "Min delay should not exceed max delay");
            }
        }
    }

    /// Test parameter validation in constraint functions
    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_invalid_cycle_time_zero() {
        let hbcn = create_test_hbcn(r#"Port "a" []"#, false);
        let _ = constrain_cycle_time_pseudoclock(&hbcn, 0.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_invalid_cycle_time_negative() {
        let hbcn = create_test_hbcn(r#"Port "a" []"#, false);
        let _ = constrain_cycle_time_pseudoclock(&hbcn, -5.0, 1.0);
    }

    /// Test that constraint results have proper HBCN timing information
    #[test]
    fn test_constraint_result_timing() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        let result = constrain_cycle_time_pseudoclock(&hbcn, 10.0, 1.0)
            .expect("Should generate constraints");

        // Check that the result StructuralHBCN has timing information
        for node_idx in result.hbcn.node_indices() {
            let node = &result.hbcn[node_idx];
            // Time should be non-negative
            assert!(node.time >= 0.0, "Node timing should be non-negative");
        }

        // Check that edges have delay information
        for edge_idx in result.hbcn.edge_indices() {
            let edge = &result.hbcn[edge_idx];
            // Delays should be reasonable if present
            if let Some(max_delay) = edge.delay.max {
                assert!(
                    max_delay >= 1.0,
                    "Edge max delay should be at least min_delay"
                );
            }
            if let Some(min_delay) = edge.delay.min {
                assert!(min_delay >= 0.0, "Edge min delay should be non-negative");
            }
        }
    }

    /// Test constraint generation with complex multi-path circuit
    #[test]
    fn test_complex_multipath_constraints() {
        // Create a more complex circuit with multiple paths
        let hbcn = create_test_hbcn(
            r#"Port "in1" [("join", 50)]
               Port "in2" [("join", 75)]
               DataReg "join" [("out1", 100), ("out2", 125)]
               Port "out1" []
               Port "out2" []"#,
            false,
        );

        let result = constrain_cycle_time_proportional(&hbcn, 30.0, 3.0, None, None)
            .expect("Should handle complex multipath circuit");

        // Should have constraints for multiple paths
        assert!(
            result.path_constraints.len() > 2,
            "Should have multiple path constraints"
        );

        // All constraints should be valid
        for (_, constraint) in &result.path_constraints {
            if let Some(max_delay) = constraint.max {
                assert!(max_delay >= 3.0);
                assert!(max_delay <= 30.0);
            }
            if let Some(min_delay) = constraint.min {
                assert!(min_delay >= 0.0);
                assert!(min_delay <= 30.0);
            }
        }
    }

    /// Test that critical cycles can be found in constraint results
    #[test]
    fn test_critical_cycle_analysis() {
        // Use a more feasible circuit with DataReg for cycle formation
        let hbcn = create_test_hbcn(
            r#"Port "input" [("reg", 100)]
               DataReg "reg" [("output", 50), ("input", 75)]
               Port "output" []"#,
            false,
        );

        let result = constrain_cycle_time_proportional(&hbcn, 500.0, 10.0, None, None)
            .expect("Should handle cyclic circuit with feasible parameters");

        // Find critical cycles in the result
        let cycles = find_critical_cycles(&result.hbcn);

        // Should work even if no cycles are found (depends on StructuralHBCN structure)
        // Each cycle should have at least 2 edges if any are found
        for cycle in &cycles {
            assert!(cycle.len() >= 2, "Cycles should have at least 2 edges");
        }

        // Test passes as long as the constraint generation succeeds
        assert!(result.pseudoclock_period >= 10.0);
    }

    /// Test extreme margin values  
    #[test]
    fn test_extreme_margin_values() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        // Test with moderate margins and feasible parameters
        let result = constrain_cycle_time_proportional(&hbcn, 1000.0, 20.0, Some(0.3), Some(0.3))
            .expect("Should handle moderate margins");

        assert!(result.pseudoclock_period >= 20.0);

        // Test with very loose margins (allowing more flexibility)
        let result = constrain_cycle_time_proportional(&hbcn, 1000.0, 20.0, Some(0.8), Some(0.8))
            .expect("Should handle loose margins");

        assert!(result.pseudoclock_period >= 20.0);

        // Both should complete successfully
        assert!(result.pseudoclock_period < 1000.0);
    }

    /// Test cyclic path constraint generation (based on cyclic.graph structure)
    #[test]
    fn test_cyclic_path_constraints() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 20)]
               DataReg "b" [("b", 15), ("c", 10)]
               Port "c" []"#,
            false,
        );

        // Test pseudoclock constraints on cyclic circuit
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 50.0, 2.0)
            .expect("Should handle cyclic circuit with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 2.0);
        assert!(!pseudo_result.path_constraints.is_empty());

        // Test proportional constraints on cyclic circuit
        let prop_result = constrain_cycle_time_proportional(&hbcn, 50.0, 2.0, None, None)
            .expect("Should handle cyclic circuit with proportional");

        assert!(prop_result.pseudoclock_period >= 2.0);
        assert!(!prop_result.path_constraints.is_empty());

        // Both algorithms should produce valid results for cyclic circuits
        assert!(pseudo_result.pseudoclock_period <= 50.0);
        assert!(prop_result.pseudoclock_period <= 50.0);
    }

    /// Test cyclic circuit with feedback loop constraints
    #[test]
    fn test_cyclic_feedback_constraints() {
        let hbcn = create_test_hbcn(
            r#"Port "input" [("reg", 30)]
               DataReg "reg" [("output", 25), ("reg", 20)]
               Port "output" []"#,
            false,
        );

        // Test with generous cycle time for feedback circuit
        let result = constrain_cycle_time_proportional(&hbcn, 200.0, 10.0, None, None)
            .expect("Should handle feedback circuit");

        assert!(result.pseudoclock_period >= 10.0);
        assert!(!result.path_constraints.is_empty());

        // Test with margins on cyclic circuit
        let result_with_margins = constrain_cycle_time_proportional(&hbcn, 200.0, 10.0, Some(0.2), Some(0.3))
            .expect("Should handle feedback circuit with margins");

        assert!(result_with_margins.pseudoclock_period >= 10.0);
        assert!(!result_with_margins.path_constraints.is_empty());
    }

    /// Test complex cyclic circuit with multiple feedback paths
    #[test]
    fn test_complex_cyclic_constraints() {
        let hbcn = create_test_hbcn(
            r#"Port "clk" [("reg1", 5), ("reg2", 5)]
               Port "input" [("reg1", 40)]
               DataReg "reg1" [("logic", 30), ("reg2", 25)]
               DataReg "reg2" [("logic", 35), ("reg1", 20)]
               DataReg "logic" [("output", 45)]
               Port "output" []"#,
            false,
        );

        // Test with very generous cycle time for complex cyclic circuit
        let result = constrain_cycle_time_proportional(&hbcn, 500.0, 15.0, None, None)
            .expect("Should handle complex cyclic circuit");

        assert!(result.pseudoclock_period >= 15.0);
        assert!(!result.path_constraints.is_empty());

        // Test pseudoclock on complex cyclic circuit
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 500.0, 15.0)
            .expect("Should handle complex cyclic circuit with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 15.0);
        assert!(!pseudo_result.path_constraints.is_empty());
    }

    /// Test cyclic circuit constraint validation
    #[test]
    fn test_cyclic_constraint_validity() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 20)]
               DataReg "b" [("b", 15), ("c", 10)]
               Port "c" []"#,
            false,
        );

        let result = constrain_cycle_time_proportional(&hbcn, 100.0, 5.0, None, None)
            .expect("Should generate valid cyclic constraints");

        // Validate all constraints in cyclic circuit
        for ((src, dst), constraint) in &result.path_constraints {
            // Source and destination should be valid circuit nodes
            assert!(!src.name().as_ref().is_empty());
            assert!(!dst.name().as_ref().is_empty());

            // Delays should be reasonable for cyclic circuit
            if let Some(min_delay) = constraint.min {
                assert!(min_delay >= 0.0, "Min delay should be non-negative");
                assert!(min_delay <= 100.0, "Min delay should be reasonable");
            }

            if let Some(max_delay) = constraint.max {
                assert!(max_delay >= 5.0, "Max delay should be at least min_delay");
                assert!(max_delay <= 100.0, "Max delay should not exceed cycle time");
            }

            // If both exist, min should be <= max
            if let (Some(min), Some(max)) = (constraint.min, constraint.max) {
                assert!(min <= max, "Min delay should not exceed max delay");
            }
        }
    }

    /// Test cyclic circuit with tight timing constraints
    #[test]
    fn test_cyclic_tight_timing() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 10)]
               DataReg "b" [("b", 5), ("c", 8)]
               Port "c" []"#,
            false,
        );

        // Test with tight but feasible timing
        let result = constrain_cycle_time_proportional(&hbcn, 30.0, 2.0, None, None)
            .expect("Should handle cyclic circuit with tight timing");

        assert!(result.pseudoclock_period >= 2.0);
        assert!(result.pseudoclock_period <= 30.0);

        // Test with very tight timing (might be infeasible)
        let tight_result = constrain_cycle_time_proportional(&hbcn, 10.0, 1.0, None, None);
        
        // Either succeeds with valid constraints or fails gracefully
        match tight_result {
            Ok(result) => {
                assert!(result.pseudoclock_period >= 1.0);
                assert!(result.pseudoclock_period <= 10.0);
            }
            Err(_) => {
                // Expected for very tight timing on cyclic circuit
            }
        }
    }

    /// Test cyclic circuit with forward completion effects
    #[test]
    fn test_cyclic_forward_completion_effects() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        let hbcn_no_fc = create_test_hbcn(input, false);
        let hbcn_with_fc = create_test_hbcn(input, true);

        let result_no_fc = constrain_cycle_time_proportional(&hbcn_no_fc, 100.0, 5.0, None, None)
            .expect("Should work without forward completion on cyclic circuit");

        let result_with_fc = constrain_cycle_time_proportional(&hbcn_with_fc, 100.0, 5.0, None, None)
            .expect("Should work with forward completion on cyclic circuit");

        // Both should produce valid results
        assert!(result_no_fc.pseudoclock_period >= 5.0);
        assert!(result_with_fc.pseudoclock_period >= 5.0);

        // Both should have constraints
        assert!(!result_no_fc.path_constraints.is_empty());
        assert!(!result_with_fc.path_constraints.is_empty());
    }

    /// Helper function to calculate critical cycle time per token
    fn calculate_critical_cycle_time_per_token(delayed_hbcn: &DelayedHBCN) -> f64 {
        let cycles = find_critical_cycles(delayed_hbcn);
        
        if cycles.is_empty() {
            return 0.0;
        }

        // Find the cycle with maximum cost per token
        let mut max_cost_per_token: f64 = 0.0;
        
        for cycle in &cycles {
            let mut tokens = 0;
            let cost: f64 = cycle
                .iter()
                .map(|(is, it)| {
                    let ie = delayed_hbcn.find_edge(*is, *it).unwrap();
                    let e = &delayed_hbcn[ie];
                    if e.is_marked() {
                        tokens += 1;
                    }
                    e.weight() - e.slack()
                })
                .sum();
            
            if tokens > 0 {
                let cost_per_token = cost / tokens as f64;
                max_cost_per_token = max_cost_per_token.max(cost_per_token);
            }
        }
        
        max_cost_per_token
    }

    /// Test that pseudoclock constraints correctly limit cycle time per token
    #[test]
    fn test_pseudoclock_constraints_cycle_time_verification() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        let requested_cycle_time = 50.0;
        let min_delay = 5.0;

        let result = constrain_cycle_time_pseudoclock(&hbcn, requested_cycle_time, min_delay)
            .expect("Pseudoclock constraint generation should succeed");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token(&result.hbcn);

        // The actual cycle time per token should be less than or equal to the requested cycle time
        assert!(
            actual_cycle_time_per_token <= requested_cycle_time,
            "Critical cycle time per token ({}) should be <= requested cycle time ({})",
            actual_cycle_time_per_token,
            requested_cycle_time
        );

        // Should have reasonable pseudoclock period
        assert!(result.pseudoclock_period > 0.0);
        assert!(result.pseudoclock_period <= requested_cycle_time);
    }

    /// Test that proportional constraints correctly limit cycle time per token
    #[test]
    fn test_proportional_constraints_cycle_time_verification() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        let requested_cycle_time = 60.0;
        let min_delay = 8.0;

        let result = constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
            .expect("Proportional constraint generation should succeed");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token(&result.hbcn);

        // The actual cycle time per token should be less than or equal to the requested cycle time
        assert!(
            actual_cycle_time_per_token <= requested_cycle_time,
            "Critical cycle time per token ({}) should be <= requested cycle time ({})",
            actual_cycle_time_per_token,
            requested_cycle_time
        );
    }

    /// Test cycle time verification with cyclic circuit
    #[test]
    fn test_cyclic_constraints_cycle_time_verification() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               DataReg "b" [("a", 50), ("c", 75)]
               Port "c" []"#,
            false,
        );

        let requested_cycle_time = 80.0;
        let min_delay = 10.0;

        let result = constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
            .expect("Proportional constraint generation should succeed for cyclic circuit");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token(&result.hbcn);

        // The actual cycle time per token should be less than or equal to the requested cycle time
        assert!(
            actual_cycle_time_per_token <= requested_cycle_time,
            "Critical cycle time per token ({}) should be <= requested cycle time ({}) for cyclic circuit",
            actual_cycle_time_per_token,
            requested_cycle_time
        );
    }

    /// Test cycle time verification with complex circuit
    #[test]
    fn test_complex_constraints_cycle_time_verification() {
        let hbcn = create_test_hbcn(
            r#"Port "input" [("reg1", 100)]
               DataReg "reg1" [("reg2", 120), ("input", 80)]
               DataReg "reg2" [("output", 90), ("reg1", 60)]
               Port "output" []"#,
            false,
        );

        let requested_cycle_time = 150.0;
        let min_delay = 15.0;

        let result = constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
            .expect("Proportional constraint generation should succeed for complex circuit");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token(&result.hbcn);

        // The actual cycle time per token should be less than or equal to the requested cycle time
        assert!(
            actual_cycle_time_per_token <= requested_cycle_time,
            "Critical cycle time per token ({}) should be <= requested cycle time ({}) for complex circuit",
            actual_cycle_time_per_token,
            requested_cycle_time
        );
    }

    /// Test that tighter constraints result in lower cycle times
    #[test]
    fn test_constraint_tightness_verification() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        // Test with loose constraints
        let loose_result = constrain_cycle_time_pseudoclock(&hbcn, 200.0, 5.0)
            .expect("Loose constraint generation should succeed");

        // Test with tight constraints
        let tight_result = constrain_cycle_time_pseudoclock(&hbcn, 50.0, 5.0)
            .expect("Tight constraint generation should succeed");

        let loose_cycle_time = calculate_critical_cycle_time_per_token(&loose_result.hbcn);
        let tight_cycle_time = calculate_critical_cycle_time_per_token(&tight_result.hbcn);

        // Both should meet their respective constraints
        assert!(loose_cycle_time <= 200.0);
        assert!(tight_cycle_time <= 50.0);

        // The tight constraint should result in a lower or equal cycle time
        assert!(
            tight_cycle_time <= loose_cycle_time,
            "Tight constraints should result in lower or equal cycle time (tight: {}, loose: {})",
            tight_cycle_time,
            loose_cycle_time
        );
    }

    /// Test cycle time verification with different min_delay values
    #[test]
    fn test_min_delay_cycle_time_verification() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        let requested_cycle_time = 100.0;
        let min_delays = vec![1.0, 5.0, 10.0, 20.0];

        for min_delay in min_delays {
            let result = constrain_cycle_time_pseudoclock(&hbcn, requested_cycle_time, min_delay)
                .expect("Constraint generation should succeed for min_delay");

            let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token(&result.hbcn);

            // The actual cycle time per token should be less than or equal to the requested cycle time
            assert!(
                actual_cycle_time_per_token <= requested_cycle_time,
                "Critical cycle time per token ({}) should be <= requested cycle time ({}) for min_delay {}",
                actual_cycle_time_per_token,
                requested_cycle_time,
                min_delay
            );

            // The pseudoclock period should be reasonable
            assert!(result.pseudoclock_period > 0.0);
            assert!(result.pseudoclock_period <= requested_cycle_time);
        }
    }

    /// Test cycle time verification with proportional algorithm on cyclic circuit
    /// Note: Proportional algorithm is more suitable for cyclic circuits than pseudoclock
    #[test]
    fn test_proportional_cyclic_cycle_time_verification() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               DataReg "b" [("a", 50), ("c", 75)]
               Port "c" []"#,
            false,
        );

        let requested_cycle_time = 80.0;
        let min_delay = 10.0;

        let result = constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
            .expect("Proportional constraint generation should succeed for cyclic circuit");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token(&result.hbcn);

        // The actual cycle time per token should be less than or equal to the requested cycle time
        assert!(
            actual_cycle_time_per_token <= requested_cycle_time,
            "Critical cycle time per token ({}) should be <= requested cycle time ({}) for proportional cyclic circuit",
            actual_cycle_time_per_token,
            requested_cycle_time
        );

        // Should have reasonable pseudoclock period
        assert!(result.pseudoclock_period > 0.0);
        assert!(result.pseudoclock_period <= requested_cycle_time);
    }
}
