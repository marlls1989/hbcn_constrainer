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

        let result = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 10.0, 1.0)
            .expect("Pseudoclock constraint generation should succeed");

        // Pseudoclock period should be reasonable
        assert!(result.pseudoclock_period >= 1.0);
        assert!(result.pseudoclock_period <= 10.0);

        // Should have path constraints
        assert!(result.hbcn.edge_count() > 0);

        // All max delays should be valid
        for ie in result.hbcn.edge_indices() {
            let constraint = &result.hbcn[ie].delay;
            assert!(
                constraint.max >= 1.0,
                "Max delay should be at least min_delay"
            );
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

        let result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 10.0, 1.0, None, None)
                .expect("Proportional constraint generation should succeed");

        // Pseudoclock period should be reasonable
        assert!(result.pseudoclock_period >= 1.0);

        // Should have path constraints
        assert!(result.hbcn.edge_count() > 0);

        // DelayPair.min constraints are currently only generated when margins are
        // provided. There is no guarantee that any constraint will have a min when
        // no margins are provided. However, all constraints must have max >= min_delay.
        // Verify this for all constraints.
        for ie in result.hbcn.edge_indices() {
            let constraint = &result.hbcn[ie].delay;
            assert!(
                constraint.max >= 1.0,
                "All constraints must have max delay >= min_delay ({} >= {})",
                constraint.max,
                1.0
            );
            if let Some(min) = constraint.min {
                assert!(
                    min <= constraint.max,
                    "Min delay should not exceed max delay"
                );
            }
        }
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
        let result = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 0.1, 10.0);
        assert!(result.is_err(), "Should fail with infeasible constraints");

        // Try proportional with impossible parameters
        let result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 0.1, 10.0, None, None);
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
        let result_with_margin = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            500.0,
            10.0,
            None,
            Some(0.5),
        )
        .expect("Should succeed with forward margin");

        let result_without_margin = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn, 500.0, 10.0, None, None,
        )
        .expect("Should succeed without margin");

        // Both results should be valid
        assert!(result_with_margin.pseudoclock_period >= 10.0);
        assert!(result_without_margin.pseudoclock_period >= 10.0);

        // Test with backward margin
        let result_backward = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            500.0,
            10.0,
            Some(0.5),
            None,
        )
        .expect("Should succeed with backward margin");

        assert!(result_backward.pseudoclock_period >= 10.0);

        // At least one should produce some constraints
        let total_constraints = result_with_margin.hbcn.edge_count()
            + result_without_margin.hbcn.edge_count()
            + result_backward.hbcn.edge_count();
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
        let pseudo_result =
            crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 20.0, 2.0)
                .expect("Should handle DataReg with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 2.0);
        assert!(pseudo_result.hbcn.edge_count() > 0);

        // Should work with proportional
        let prop_result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 20.0, 2.0, None, None)
                .expect("Should handle DataReg with proportional");

        assert!(prop_result.pseudoclock_period >= 2.0);
        assert!(prop_result.hbcn.edge_count() > 0);
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
        let result = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 100.0, 5.0)
            .expect("Should handle minimal circuit");

        assert!(result.pseudoclock_period >= 5.0);
        // Path constraints may or may not exist for minimal circuits
    }

    /// Test that forward completion affects constraint generation
    #[test]
    fn test_forward_completion_effects() {
        // A register with high fan-in (4 inputs) and small edge weights makes the
        // completion cost exceed the virtual delay, so `forward_completion` provably
        // raises forward-place weights. If the flag were ignored, the two HBCNs — and
        // therefore their generated constraints — would be byte-for-byte identical.
        let input = r#"Port "i1" [("m", 1)]
                      Port "i2" [("m", 1)]
                      Port "i3" [("m", 1)]
                      Port "i4" [("m", 1)]
                      DataReg "m" [("o", 1)]
                      Port "o" []"#;

        let hbcn_no_fc = create_test_hbcn(input, false);
        let hbcn_with_fc = create_test_hbcn(input, true);

        // The flag must actually change the model: total place weight must increase.
        let sum_no: f64 = hbcn_no_fc
            .edge_indices()
            .map(|e| hbcn_no_fc[e].weight)
            .sum();
        let sum_fc: f64 = hbcn_with_fc
            .edge_indices()
            .map(|e| hbcn_with_fc[e].weight)
            .sum();
        assert!(
            sum_fc > sum_no,
            "forward_completion should raise forward-place weights (no_fc={sum_no}, fc={sum_fc})"
        );

        let result_no_fc = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn_no_fc,
            300.0,
            10.0,
            None,
            None,
        )
        .expect("Should work without forward completion");

        let result_with_fc = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn_with_fc,
            300.0,
            10.0,
            None,
            None,
        )
        .expect("Should work with forward completion");

        // The heavier completion costs must surface as different generated constraints.
        let delays_no_fc: Vec<_> = result_no_fc
            .hbcn
            .edge_indices()
            .map(|ie| result_no_fc.hbcn[ie].delay.clone())
            .collect();
        let delays_with_fc: Vec<_> = result_with_fc
            .hbcn
            .edge_indices()
            .map(|ie| result_with_fc.hbcn[ie].delay.clone())
            .collect();
        assert_ne!(
            delays_no_fc, delays_with_fc,
            "forward_completion should change the generated path constraints"
        );

        assert!(result_no_fc.pseudoclock_period >= 10.0);
        assert!(result_with_fc.pseudoclock_period >= 10.0);
        assert!(result_no_fc.hbcn.edge_count() > 0);
        assert!(result_with_fc.hbcn.edge_count() > 0);
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

        let result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 50.0, 5.0, None, None)
                .expect("Should generate valid constraints");

        // Validate all constraints
        for ie in result.hbcn.edge_indices() {
            let (is, id) = result.hbcn.edge_endpoints(ie).unwrap();
            let src: &CircuitNode = result.hbcn[is].as_ref();
            let dst: &CircuitNode = result.hbcn[id].as_ref();
            let constraint = &result.hbcn[ie].delay;
            // Source and destination should be valid circuit nodes
            assert!(!src.name().as_ref().is_empty());
            assert!(!dst.name().as_ref().is_empty());

            // Delays should be reasonable
            if let Some(min_delay) = constraint.min {
                assert!(min_delay >= 0.0, "Min delay should be non-negative");
                assert!(min_delay <= 50.0, "Min delay should be reasonable");
            }

            assert!(
                constraint.max >= 5.0,
                "Max delay should be at least min_delay"
            );
            assert!(
                constraint.max <= 50.0,
                "Max delay should not exceed cycle time"
            );

            // If min exists, min should be <= max
            if let Some(min) = constraint.min {
                assert!(
                    min <= constraint.max,
                    "Min delay should not exceed max delay"
                );
            }
        }
    }

    /// Test parameter validation in constraint functions
    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_invalid_cycle_time_zero() {
        let hbcn = create_test_hbcn(r#"Port "a" []"#, false);
        let _ = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 0.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_invalid_cycle_time_negative() {
        let hbcn = create_test_hbcn(r#"Port "a" []"#, false);
        let _ = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, -5.0, 1.0);
    }

    /// Test that constraint results have proper HBCN timing information
    #[test]
    fn test_constraint_result_timing() {
        let hbcn = create_test_hbcn(
            r#"Port "a" [("b", 100)]
               Port "b" []"#,
            false,
        );

        let result = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 10.0, 1.0)
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
            // Delays should be reasonable (max is always present now)
            assert!(
                edge.delay.max >= 1.0,
                "Edge max delay should be at least min_delay"
            );
            if let Some(min_delay) = edge.delay.min {
                assert!(min_delay >= 0.0, "Edge min delay should be non-negative");
                assert!(
                    min_delay <= edge.delay.max,
                    "Edge min delay should be <= max delay: min_delay={}, max={}",
                    min_delay,
                    edge.delay.max
                );
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

        let result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 30.0, 3.0, None, None)
                .expect("Should handle complex multipath circuit");

        // Should have constraints for multiple paths
        assert!(
            result.hbcn.edge_count() > 2,
            "Should have multiple path constraints"
        );

        // All constraints should be valid
        for ie in result.hbcn.edge_indices() {
            let constraint = &result.hbcn[ie].delay;
            assert!(constraint.max >= 3.0);
            assert!(constraint.max <= 30.0);
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

        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn, 500.0, 10.0, None, None,
        )
        .expect("Should handle cyclic circuit with feasible parameters");

        // Find critical cycles in the result
        let cycles = crate::analyse::hbcn::find_critical_cycles(&result.hbcn);

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
        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            1000.0,
            20.0,
            Some(0.3),
            Some(0.3),
        )
        .expect("Should handle moderate margins");

        assert!(result.pseudoclock_period >= 20.0);

        // Test with very loose margins (allowing more flexibility)
        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            1000.0,
            20.0,
            Some(0.8),
            Some(0.8),
        )
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
        let pseudo_result =
            crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 50.0, 2.0)
                .expect("Should handle cyclic circuit with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 2.0);
        assert!(pseudo_result.hbcn.edge_count() > 0);

        // Test proportional constraints on cyclic circuit
        let prop_result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 50.0, 2.0, None, None)
                .expect("Should handle cyclic circuit with proportional");

        assert!(prop_result.pseudoclock_period >= 2.0);
        assert!(prop_result.hbcn.edge_count() > 0);

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
        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn, 200.0, 10.0, None, None,
        )
        .expect("Should handle feedback circuit");

        assert!(result.pseudoclock_period >= 10.0);
        assert!(result.hbcn.edge_count() > 0);

        // Test with margins on cyclic circuit
        let result_with_margins = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            200.0,
            10.0,
            Some(0.2),
            Some(0.3),
        )
        .expect("Should handle feedback circuit with margins");

        assert!(result_with_margins.pseudoclock_period >= 10.0);
        assert!(result_with_margins.hbcn.edge_count() > 0);
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
        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn, 500.0, 15.0, None, None,
        )
        .expect("Should handle complex cyclic circuit");

        assert!(result.pseudoclock_period >= 15.0);
        assert!(result.hbcn.edge_count() > 0);

        // Test pseudoclock on complex cyclic circuit
        let pseudo_result =
            crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 500.0, 15.0)
                .expect("Should handle complex cyclic circuit with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 15.0);
        assert!(pseudo_result.hbcn.edge_count() > 0);
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

        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn, 100.0, 5.0, None, None,
        )
        .expect("Should generate valid cyclic constraints");

        // Validate all constraints in cyclic circuit
        for ie in result.hbcn.edge_indices() {
            let (is, id) = result.hbcn.edge_endpoints(ie).unwrap();
            let src: &CircuitNode = result.hbcn[is].as_ref();
            let dst: &CircuitNode = result.hbcn[id].as_ref();
            let constraint = &result.hbcn[ie].delay;
            // Source and destination should be valid circuit nodes
            assert!(!src.name().as_ref().is_empty());
            assert!(!dst.name().as_ref().is_empty());

            // Delays should be reasonable for cyclic circuit
            if let Some(min_delay) = constraint.min {
                assert!(min_delay >= 0.0, "Min delay should be non-negative");
                assert!(min_delay <= 100.0, "Min delay should be reasonable");
            }

            // Max is now mandatory
            assert!(
                constraint.max >= 5.0,
                "Max delay should be at least min_delay"
            );
            assert!(
                constraint.max <= 100.0,
                "Max delay should not exceed cycle time"
            );

            // If min exists, min should be <= max
            if let Some(min) = constraint.min {
                assert!(
                    min <= constraint.max,
                    "Min delay should not exceed max delay"
                );
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
        let result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 30.0, 2.0, None, None)
                .expect("Should handle cyclic circuit with tight timing");

        assert!(result.pseudoclock_period >= 2.0);
        assert!(result.pseudoclock_period <= 30.0);

        // Test with very tight timing (might be infeasible)
        let tight_result =
            crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 10.0, 1.0, None, None);

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

        let result_no_fc = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn_no_fc,
            100.0,
            5.0,
            None,
            None,
        )
        .expect("Should work without forward completion on cyclic circuit");

        let result_with_fc = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn_with_fc,
            100.0,
            5.0,
            None,
            None,
        )
        .expect("Should work with forward completion on cyclic circuit");

        // Both should produce valid results
        assert!(result_no_fc.pseudoclock_period >= 5.0);
        assert!(result_with_fc.pseudoclock_period >= 5.0);

        // Both should have constraints
        assert!(result_no_fc.hbcn.edge_count() > 0);
        assert!(result_with_fc.hbcn.edge_count() > 0);
    }

    /// Helper function to calculate critical cycle time per token
    fn calculate_critical_cycle_time_per_token(delayed_hbcn: &SolvedHBCN) -> f64 {
        let cycles = crate::analyse::hbcn::find_critical_cycles(delayed_hbcn);

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

        let result = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(
            &hbcn,
            requested_cycle_time,
            min_delay,
        )
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

        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            requested_cycle_time,
            min_delay,
            None,
            None,
        )
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

        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            requested_cycle_time,
            min_delay,
            None,
            None,
        )
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

        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            requested_cycle_time,
            min_delay,
            None,
            None,
        )
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
        let loose_result =
            crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 200.0, 5.0)
                .expect("Loose constraint generation should succeed");

        // Test with tight constraints
        let tight_result =
            crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 50.0, 5.0)
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
            let result = crate::constrain::hbcn::constrain_cycle_time_pseudoclock(
                &hbcn,
                requested_cycle_time,
                min_delay,
            )
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

        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn,
            requested_cycle_time,
            min_delay,
            None,
            None,
        )
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

    /// A `.hbcn` channel whose forward-data and forward-spacer places carry different weights
    /// must not collapse in the proportional LP: sharing one delay variable would force
    /// `(w_data - w_spacer) * factor == 0`, driving `factor` to zero (all maxes at the floor).
    /// Per-place variables keep the two forward maxes distinct and proportional to their weights.
    #[test]
    fn proportional_keeps_distinct_forward_delays() {
        use crate::hbcn::Transition;
        use crate::hbcn::parser::parse_hbcn;

        let input = r#"
            * +{port:a} => +{reg1} : 100
              -{port:a} => -{reg1} : 40
              +{reg1} => -{port:a} : 30
              -{reg1} => +{port:a} : 30
        "#;
        let hbcn = parse_hbcn(input).expect("Should parse distinct-delay HBCN");
        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn, 1000.0, 10.0, None, None,
        )
        .expect("Should constrain distinct-delay channel");

        // Single channel: exactly one forward-data and one forward-spacer place.
        let mut data_max = 0.0_f64;
        let mut spacer_max = 0.0_f64;
        for ie in result.hbcn.edge_indices() {
            let (s, d) = result.hbcn.edge_endpoints(ie).unwrap();
            let max = result.hbcn[ie].delay.max;
            match (&result.hbcn[s].transition, &result.hbcn[d].transition) {
                (Transition::Data(_), Transition::Data(_)) => data_max = max,
                (Transition::Spacer(_), Transition::Spacer(_)) => spacer_max = max,
                _ => {}
            }
        }

        assert!(
            data_max > 10.0 && spacer_max > 10.0,
            "both forward maxes should rise above the floor (data={data_max}, spacer={spacer_max}); \
             a degenerate factor==0 would pin them to the floor"
        );
        assert!(
            data_max > spacer_max,
            "the heavier forward-data weight must yield a larger max than forward-spacer \
             (data={data_max}, spacer={spacer_max}); equal would mean the LP collapsed them"
        );
    }

    /// End-to-end: a distinct-delay `.hbcn` through proportional constrain emits SDC with both
    /// rising (data) and falling (spacer) `-..._through` qualifiers.
    #[test]
    fn constrain_distinct_delays_emit_rise_and_fall_sdc() {
        use crate::hbcn::parser::parse_hbcn;
        use std::io::Cursor;

        let input = r#"
            * +{port:a} => +{reg1} : 100
              -{port:a} => -{reg1} : 40
              +{reg1} => -{port:a} : 30
              -{reg1} => +{port:a} : 30
        "#;
        let hbcn = parse_hbcn(input).expect("Should parse");
        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(
            &hbcn, 1000.0, 10.0, None, None,
        )
        .expect("Should constrain");

        let mut out = Cursor::new(Vec::new());
        crate::constrain::sdc::write_path_constraints(
            &mut out,
            &result.hbcn,
            result.pseudoclock_period,
        )
        .expect("Should write SDC");
        let sdc = String::from_utf8(out.into_inner()).expect("valid UTF-8");

        assert!(
            sdc.contains("-rise_through"),
            "SDC should constrain rising (data) paths:\n{sdc}"
        );
        assert!(
            sdc.contains("-fall_through"),
            "SDC should constrain falling (spacer) paths:\n{sdc}"
        );
        assert!(
            sdc.contains("set_max_delay"),
            "SDC should emit max-delay constraints:\n{sdc}"
        );
    }
}
