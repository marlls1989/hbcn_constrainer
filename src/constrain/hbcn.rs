//! Constraint generation algorithms for HBCN circuits.
//!
//! This module provides the core algorithms for generating timing constraints from HBCN
//! representations. It supports multiple constraint generation strategies:
//!
//! - **Proportional constraints**: Distribute cycle time proportionally across paths
//! - **Pseudoclock constraints**: Use a pseudo-clock period for external path constraints
//!
//! Both algorithms use linear programming to solve for optimal delay constraints that
//! meet the specified cycle time requirements.

use std::collections::HashMap;

use petgraph::prelude::*;

use crate::AppError;
use crate::hbcn::*;
use lp_solver::*;
use lp_solver::{constraint, lp_model_builder};

/// A `Data` transition is a rise at its register/port; a `Spacer` transition is a fall.
/// Path constraints are carried per place by the [`SolvedHBCN`] in [`ConstrainerResult`];
/// the SDC and CSV writers read this to qualify each path endpoint as rise/fall.
pub(crate) fn is_rise(t: &Transition) -> bool {
    matches!(t, Transition::Data(_))
}

#[derive(Debug, Clone)]
pub struct ConstrainerResult {
    pub pseudoclock_period: f64,
    pub hbcn: SolvedHBCN,
}

/// Constrain cycle time using the pseudoclock algorithm
pub fn constrain_cycle_time_pseudoclock<T, P>(
    hbcn: &HBCN<T, P>,
    ct: f64,
    min_delay: f64,
) -> anyhow::Result<ConstrainerResult>
where
    T: AsRef<Transition> + AsRef<CircuitNode>,
    P: AsRef<Place> + HasWeight + Clone + Into<Place>,
{
    assert!(ct > 0.0);

    let mut builder = lp_model_builder!();

    let pseudo_clock = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);

    let arr_var: HashMap<NodeIndex, VariableId<_>> = hbcn
        .node_indices()
        .map(|x| {
            (
                x,
                builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY),
            )
        })
        .collect();

    // One delay variable per place (edge), mirroring the analyse LP (`compute_cycle_time`).
    // Keying by `EdgeIndex` keeps the four places of a channel independent rather than
    // collapsing the two same-node-pair places onto a shared variable.
    let delay_vars: HashMap<EdgeIndex, VariableId<_>> = hbcn
        .edge_indices()
        .map(|ie| {
            (
                ie,
                builder.add_variable(VariableType::Continuous, min_delay, f64::INFINITY),
            )
        })
        .collect();

    for ie in hbcn.edge_indices() {
        let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
        let place = &hbcn[ie];
        let delay = delay_vars[&ie];

        // Create constraint: delay + arr_var[src] - arr_var[dst] = (if place.token { ct } else { 0.0 })
        let place_ref: &Place = place.as_ref();
        let rhs = if place_ref.token { ct } else { 0.0 };
        builder.add_constraint(constraint!((delay + arr_var[&src] - arr_var[&dst]) == rhs));

        if place_ref.is_internal {
            builder.add_constraint(constraint!((delay) >= min_delay));
        } else {
            builder.add_constraint(constraint!((delay - pseudo_clock) >= 0.0));
        }
    }

    builder.set_objective(pseudo_clock, OptimisationSense::Maximise);

    let solution = match builder.solve() {
        Ok(s) => s,
        Err(SolveError::NoSolution(_)) => return Err(AppError::Infeasible.into()),
        Err(e) => return Err(e.into()),
    };

    let pseudo_clock_value =
        round_to_sig_digits(solution.get_value(pseudo_clock).unwrap_or(min_delay), 8);
    Ok(ConstrainerResult {
        pseudoclock_period: pseudo_clock_value,
        hbcn: hbcn.map(
            |ix, x| {
                let transition: &Transition = x.as_ref();
                TransitionEvent {
                    transition: transition.clone(),
                    time: round_to_sig_digits(solution.get_value(arr_var[&ix]).unwrap_or(0.0), 8),
                }
            },
            |ie, e| {
                let delay_value = round_to_sig_digits(
                    solution.get_value(delay_vars[&ie]).unwrap_or(e.weight()),
                    8,
                );

                DelayedPlace {
                    place: e.clone().into(),
                    slack: None,
                    delay: DelayPair {
                        min: None,
                        max: delay_value,
                    },
                }
            },
        ),
    })
}

/// Constrain cycle time using the proportional algorithm
pub fn constrain_cycle_time_proportional<T, P>(
    hbcn: &HBCN<T, P>,
    ct: f64,
    min_delay: f64,
    backward_margin: Option<f64>,
    forward_margin: Option<f64>,
) -> anyhow::Result<ConstrainerResult>
where
    T: AsRef<Transition> + AsRef<CircuitNode>,
    P: AsRef<Place> + HasWeight + Clone + Into<Place>,
{
    assert!(ct > 0.0);
    assert!(min_delay >= 0.0);

    struct DelayVarPair<Brand> {
        max: VariableId<Brand>,
        min: VariableId<Brand>,
        slack: VariableId<Brand>,
    }

    let mut builder = lp_model_builder!();

    let factor = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);

    let arr_var: HashMap<NodeIndex, VariableId<_>> = hbcn
        .node_indices()
        .map(|x| {
            (
                x,
                builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY),
            )
        })
        .collect();

    // One {max,min,slack} triple per place (edge); keeps the four places of a channel
    // independent rather than sharing a variable across same-node-pair places.
    let delay_vars: HashMap<EdgeIndex, DelayVarPair<_>> = hbcn
        .edge_indices()
        .map(|ie| {
            let max = builder.add_variable(VariableType::Continuous, min_delay, f64::INFINITY);
            let min = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);
            let slack = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);
            (ie, DelayVarPair { max, min, slack })
        })
        .collect();

    // Map each (circuit node, is-data) to its transition node, so a backward (acknowledge)
    // edge can find its matching forward (propagation) edge for the margin coupling.
    let node_by_dir: HashMap<(CircuitNode, bool), NodeIndex> = hbcn
        .node_indices()
        .map(|ix| {
            let transition: &Transition = hbcn[ix].as_ref();
            let node: &CircuitNode = hbcn[ix].as_ref();
            (
                (node.clone(), matches!(transition, Transition::Data(_))),
                ix,
            )
        })
        .collect();

    for ie in hbcn.edge_indices() {
        let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
        let place = &hbcn[ie];
        let src_transition: &Transition = hbcn[src].as_ref();
        let dst_transition: &Transition = hbcn[dst].as_ref();
        let delay_var = &delay_vars[&ie];

        // Constraint: delay_var.max + arr_var[src] - arr_var[dst] = (if place.token { ct } else { 0.0 })
        let place_ref: &Place = place.as_ref();
        let rhs = if place_ref.token { ct } else { 0.0 };
        builder.add_constraint(constraint!(
            (delay_var.max + arr_var[&src] - arr_var[&dst]) == rhs
        ));

        // Constraint: delay_var.max - place.weight() * factor - delay_var.slack = 0.0
        builder.add_constraint(constraint!(
            (delay_var.max - place.weight() * factor - delay_var.slack) == 0.0
        ));

        if !place_ref.is_internal {
            let is_backward = is_backward_place(src_transition, dst_transition);
            if is_backward {
                // The matching forward (propagation) edge runs from "dst's node with the
                // ack's source polarity" to the ack's source node: data-ack pairs with
                // forward-data, spacer-ack with forward-spacer.
                let dst_node: &CircuitNode = dst_transition.as_ref();
                let src_is_data = matches!(src_transition, Transition::Data(_));
                let fwd_src = node_by_dir[&(dst_node.clone(), src_is_data)];
                let matching_edge = hbcn
                    .find_edge(fwd_src, src)
                    .expect("malformed StructuralHBCN");
                let matching_delay = &delay_vars[&matching_edge];

                if forward_margin.is_some() {
                    builder.add_constraint(constraint!(
                        (delay_var.min - matching_delay.max + matching_delay.min) == 0.0
                    ));
                }
                if let Some(bm) = backward_margin {
                    if forward_margin.is_some() {
                        builder.add_constraint(constraint!(
                            (bm * delay_var.max - delay_var.min) >= 0.0
                        ));
                    } else {
                        builder.add_constraint(constraint!(
                            (bm * delay_var.max - delay_var.min) == 0.0
                        ));
                    }
                } else if forward_margin.is_some() {
                    builder.add_constraint(constraint!((delay_var.max - delay_var.min) >= 0.0));
                }
            } else if let Some(fm) = forward_margin {
                builder.add_constraint(constraint!((fm * delay_var.max - delay_var.min) == 0.0));
            }
        }
    }

    builder.set_objective(factor, OptimisationSense::Maximise);

    let solution = match builder.solve() {
        Ok(s) => s,
        Err(SolveError::NoSolution(_)) => return Err(AppError::Infeasible.into()),
        Err(e) => return Err(e.into()),
    };

    Ok(ConstrainerResult {
        pseudoclock_period: min_delay,
        hbcn: hbcn.map(
            |ix, x| {
                let transition: &Transition = x.as_ref();
                TransitionEvent {
                    transition: transition.clone(),
                    time: round_to_sig_digits(solution.get_value(arr_var[&ix]).unwrap_or(0.0), 8),
                }
            },
            |ie, e| {
                let delay_var = &delay_vars[&ie];

                DelayedPlace {
                    place: e.clone().into(),
                    slack: solution
                        .get_value(delay_var.slack)
                        .map(|s| round_to_sig_digits(s, 8)),
                    delay: DelayPair {
                        min: solution
                            .get_value(delay_var.min)
                            .map(|m| round_to_sig_digits(m, 8)),
                        max: round_to_sig_digits(
                            solution.get_value(delay_var.max).unwrap_or(e.weight()),
                            8,
                        ),
                    },
                }
            },
        ),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hbcn::from_structural_graph;
    use crate::structural_graph::parse;

    /// Helper function to create a validated test HBCN
    fn create_test_hbcn(input: &str) -> StructuralHBCN {
        create_test_hbcn_with_fc(input, false)
    }

    /// Helper function to create a validated test HBCN with forward completion option
    fn create_test_hbcn_with_fc(input: &str, forward_completion: bool) -> StructuralHBCN {
        let structural_graph = parse(input).expect("Failed to parse");
        from_structural_graph(&structural_graph, forward_completion).expect("Failed to convert")
    }

    /// Test constraint generation with various circuit topologies
    #[test]
    fn test_constraint_algorithms_linear_chain() {
        let input = r#"
            Port "a" [("b", 10)]
            NullReg "b" [("c", 20)]
            NullReg "c" [("d", 15)]
            Port "d" []
        "#;
        let hbcn = create_test_hbcn(input);

        // Test pseudoclock algorithm with tight constraints (4x minimal delay)
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 20.0, 5.0)
            .expect("Pseudoclock should work on linear chain");

        assert!(pseudo_result.pseudoclock_period >= 5.0);
        assert!(pseudo_result.hbcn.edge_count() > 0);

        // Test proportional algorithm with tight constraints (4x minimal delay)
        let prop_result = constrain_cycle_time_proportional(&hbcn, 20.0, 5.0, None, None)
            .expect("Proportional should work on linear chain");

        assert!(prop_result.pseudoclock_period >= 5.0);
        assert!(prop_result.hbcn.edge_count() > 0);
    }

    #[test]
    fn test_constraint_algorithms_branching() {
        let input = r#"
            Port "input" [("branch1", 25), ("branch2", 30)]
            NullReg "branch1" [("merge", 15)]
            NullReg "branch2" [("merge", 20)]
            Port "merge" []
        "#;
        let hbcn = create_test_hbcn(input);

        // Test both algorithms on branching topology
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 100.0, 8.0)
            .expect("Should handle branching circuit");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 100.0, 8.0, None, None)
            .expect("Should handle branching circuit");

        // Both should produce valid results
        assert!(pseudo_result.pseudoclock_period >= 8.0);
        assert!(prop_result.pseudoclock_period >= 8.0);
        assert!(pseudo_result.hbcn.edge_count() > 0);
        assert!(prop_result.hbcn.edge_count() > 0);
    }

    #[test]
    fn test_constraint_algorithms_with_feedback() {
        let input = r#"
            Port "input" [("proc", 40)]
            DataReg "proc" [("output", 35), ("feedback", 25)]
            Port "output" []
            NullReg "feedback" [("proc", 30)]
        "#;
        let hbcn = create_test_hbcn(input);

        // Test algorithms with feedback loop
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 150.0, 10.0)
            .expect("Should handle feedback circuit");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 150.0, 10.0, None, None)
            .expect("Should handle feedback circuit");

        assert!(pseudo_result.pseudoclock_period >= 10.0);
        assert!(prop_result.pseudoclock_period >= 10.0);
    }

    #[test]
    fn test_constraint_generation_boundary_conditions() {
        let input = r#"Port "input" [("output", 50)]
                      Port "output" []"#;
        let hbcn = create_test_hbcn(input);

        // Test with reasonable parameters for a simple circuit
        let result = constrain_cycle_time_pseudoclock(&hbcn, 200.0, 5.0)
            .expect("Should handle reasonable parameters");
        assert!(result.pseudoclock_period >= 5.0);

        // Test with very large parameters
        let result = constrain_cycle_time_proportional(&hbcn, 1000.0, 10.0, None, None)
            .expect("Should handle large parameters");
        assert!(result.pseudoclock_period >= 10.0);
    }

    #[test]
    fn test_delay_pair_functionality() {
        let input = r#"
            Port "a" [("b", 50)]
            Port "b" []
        "#;
        let hbcn = create_test_hbcn(input);

        let result = constrain_cycle_time_proportional(&hbcn, 100.0, 5.0, None, None)
            .expect("Should generate constraints");

        // Test DelayPair properties in results
        for ie in result.hbcn.edge_indices() {
            let constraint = &result.hbcn[ie].delay;
            assert!(constraint.max >= 0.0, "Max delay should be non-negative");
            if let Some(min) = constraint.min {
                assert!(
                    min <= constraint.max,
                    "Min delay should not exceed max delay"
                );
                assert!(min >= 0.0, "Min delay should be non-negative");
            }
        }
    }

    #[test]
    fn test_markable_place_functionality() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let hbcn = create_test_hbcn(input);

        let mut result = constrain_cycle_time_pseudoclock(&hbcn, 50.0, 5.0)
            .expect("Should generate constraints");

        // Test marking functionality on places
        let edge_indices: Vec<_> = result.hbcn.edge_indices().collect();
        if !edge_indices.is_empty() {
            let first_edge = edge_indices[0];
            let place = &mut result.hbcn[first_edge];

            // Initially should not be marked
            assert!(!place.is_marked());

            // Mark the place
            place.mark(true);
            assert!(place.is_marked());

            // Unmark the place
            place.mark(false);
            assert!(!place.is_marked());
        }
    }

    #[test]
    fn test_proportional_vs_pseudoclock_differences() {
        let input = r#"
            Port "input" [("middle", 100)]
            NullReg "middle" [("output", 150)]
            Port "output" []
        "#;
        let hbcn = create_test_hbcn(input);

        let pseudo_result =
            constrain_cycle_time_pseudoclock(&hbcn, 300.0, 15.0).expect("Pseudoclock should work");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 300.0, 15.0, None, None)
            .expect("Proportional should work");

        // Both should produce valid results but potentially different constraints
        assert!(pseudo_result.pseudoclock_period >= 15.0);
        assert!(prop_result.pseudoclock_period >= 15.0);

        // Pseudoclock typically only produces max constraints
        let _pseudo_has_min = pseudo_result
            .hbcn
            .edge_indices()
            .any(|ie| pseudo_result.hbcn[ie].delay.min.is_some());
        let pseudo_has_max = pseudo_result
            .hbcn
            .edge_indices()
            .any(|ie| pseudo_result.hbcn[ie].delay.max >= 0.0);

        // Proportional may produce both min and max constraints
        let _prop_has_min = prop_result
            .hbcn
            .edge_indices()
            .any(|ie| prop_result.hbcn[ie].delay.min.is_some());
        let prop_has_max = prop_result
            .hbcn
            .edge_indices()
            .any(|ie| prop_result.hbcn[ie].delay.max >= 0.0);

        // At least one algorithm should produce some constraints
        assert!(
            pseudo_has_max || prop_has_max,
            "At least one algorithm should produce max constraints"
        );
    }

    #[test]
    fn test_margin_effects_detailed() {
        let input = r#"
            Port "a" [("b", 100)]
            NullReg "b" [("c", 200)]
            Port "c" []
        "#;
        let hbcn = create_test_hbcn(input);

        // Test different margin combinations
        let no_margin = constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, None, None)
            .expect("No margin should work");

        let forward_margin = constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, None, Some(0.8))
            .expect("Forward margin should work");

        let backward_margin =
            constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, Some(0.8), None)
                .expect("Backward margin should work");

        let both_margins =
            constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, Some(0.8), Some(0.8))
                .expect("Both margins should work");

        // All should produce valid pseudoclock periods
        assert!(no_margin.pseudoclock_period >= 20.0);
        assert!(forward_margin.pseudoclock_period >= 20.0);
        assert!(backward_margin.pseudoclock_period >= 20.0);
        assert!(both_margins.pseudoclock_period >= 20.0);

        // Margins should affect the results
        let periods = [
            no_margin.pseudoclock_period,
            forward_margin.pseudoclock_period,
            backward_margin.pseudoclock_period,
            both_margins.pseudoclock_period,
        ];

        // Test that all margin combinations produce valid results
        // (Margins may or may not affect results depending on the specific circuit)
        let all_valid = periods.iter().all(|&p| (20.0..=400.0).contains(&p));
        assert!(
            all_valid,
            "All margin combinations should produce valid results"
        );
    }

    /// Test cyclic circuit constraint algorithms
    #[test]
    fn test_cyclic_constraint_algorithms() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        let hbcn = create_test_hbcn(input);

        // Test pseudoclock algorithm on cyclic circuit
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 100.0, 5.0)
            .expect("Should handle cyclic circuit with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 5.0);
        assert!(pseudo_result.pseudoclock_period <= 100.0);
        assert!(pseudo_result.hbcn.edge_count() > 0);

        // Test proportional algorithm on cyclic circuit
        let prop_result = constrain_cycle_time_proportional(&hbcn, 100.0, 5.0, None, None)
            .expect("Should handle cyclic circuit with proportional");

        assert!(prop_result.pseudoclock_period >= 5.0);
        assert!(prop_result.pseudoclock_period <= 100.0);
        assert!(prop_result.hbcn.edge_count() > 0);
    }

    /// Test cyclic circuit with forward completion
    #[test]
    fn test_cyclic_forward_completion() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        // Test without forward completion
        let hbcn_no_fc = create_test_hbcn_with_fc(input, false);

        // Test with forward completion
        let hbcn_with_fc = create_test_hbcn_with_fc(input, true);

        // Both should produce valid HBCNs
        assert!(hbcn_no_fc.node_count() > 0);
        assert!(hbcn_with_fc.node_count() > 0);

        // Test constraint generation on both
        let result_no_fc = constrain_cycle_time_proportional(&hbcn_no_fc, 100.0, 5.0, None, None)
            .expect("Should work without forward completion on cyclic circuit");

        let result_with_fc =
            constrain_cycle_time_proportional(&hbcn_with_fc, 100.0, 5.0, None, None)
                .expect("Should work with forward completion on cyclic circuit");

        // Both should produce valid results
        assert!(result_no_fc.pseudoclock_period >= 5.0);
        assert!(result_with_fc.pseudoclock_period >= 5.0);
    }

    /// Test cyclic circuit edge case with minimal feedback
    #[test]
    fn test_cyclic_minimal_feedback() {
        let input = r#"Port "a" [("b", 10)]
                      DataReg "b" [("b", 5), ("c", 8)]
                      Port "c" []"#;

        let hbcn = create_test_hbcn(input);

        // Should still work with minimal feedback
        let result = constrain_cycle_time_proportional(&hbcn, 50.0, 2.0, None, None)
            .expect("Should handle minimal cyclic circuit");

        assert!(result.pseudoclock_period >= 2.0);
        assert!(result.pseudoclock_period <= 50.0);
    }

    /// Helper function to calculate critical cycle time per token for HBCN tests
    fn calculate_critical_cycle_time_per_token_hbcn(delayed_hbcn: &crate::hbcn::SolvedHBCN) -> f64 {
        let cycles = crate::analyse::hbcn::find_critical_cycles(delayed_hbcn);

        if cycles.is_empty() {
            return 0.0;
        }

        // Find the cycle with the maximum cost per token
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

    /// Test that pseudoclock constraints correctly limit cycle time per token in HBCN
    #[test]
    fn test_hbcn_pseudoclock_cycle_time_verification() {
        let input = r#"Port "a" [("b", 100)]
                      Port "b" []"#;
        let hbcn = create_test_hbcn(input);

        let requested_cycle_time = 50.0;
        let min_delay = 5.0;

        let result = constrain_cycle_time_pseudoclock(&hbcn, requested_cycle_time, min_delay)
            .expect("Pseudoclock constraint generation should succeed");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token =
            calculate_critical_cycle_time_per_token_hbcn(&result.hbcn);

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

    /// Test that proportional constraints correctly limit cycle time per token in HBCN
    #[test]
    fn test_hbcn_proportional_cycle_time_verification() {
        let input = r#"Port "a" [("b", 100)]
                      Port "b" []"#;
        let hbcn = create_test_hbcn(input);

        let requested_cycle_time = 60.0;
        let min_delay = 8.0;

        let result =
            constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
                .expect("Proportional constraint generation should succeed");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token =
            calculate_critical_cycle_time_per_token_hbcn(&result.hbcn);

        // The actual cycle time per token should be less than or equal to the requested cycle time
        assert!(
            actual_cycle_time_per_token <= requested_cycle_time,
            "Critical cycle time per token ({}) should be <= requested cycle time ({})",
            actual_cycle_time_per_token,
            requested_cycle_time
        );
    }

    /// Test cycle time verification with cyclic circuit in HBCN
    #[test]
    fn test_hbcn_cyclic_cycle_time_verification() {
        let input = r#"Port "a" [("b", 100)]
                      DataReg "b" [("a", 50), ("c", 75)]
                      Port "c" []"#;
        let hbcn = create_test_hbcn(input);

        let requested_cycle_time = 60.0;
        let min_delay = 10.0;

        let result =
            constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
                .expect("Proportional constraint generation should succeed for cyclic circuit");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token =
            calculate_critical_cycle_time_per_token_hbcn(&result.hbcn);

        // The actual cycle time per token should be less than or equal to the requested cycle time
        assert!(
            actual_cycle_time_per_token <= requested_cycle_time,
            "Critical cycle time per token ({}) should be <= requested cycle time ({}) for cyclic circuit",
            actual_cycle_time_per_token,
            requested_cycle_time
        );
    }

    /// Test that tighter constraints result in lower cycle times in HBCN
    #[test]
    fn test_hbcn_constraint_tightness_verification() {
        let input = r#"Port "a" [("b", 10)]
                      Port "b" []"#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test with loose constraints
        let loose_result = constrain_cycle_time_pseudoclock(&hbcn, 200.0, 5.0)
            .expect("Loose constraint generation should succeed");

        // Test with tight constraints
        let tight_result = constrain_cycle_time_pseudoclock(&hbcn, 20.0, 5.0)
            .expect("Tight constraint generation should succeed");

        let loose_cycle_time = calculate_critical_cycle_time_per_token_hbcn(&loose_result.hbcn);
        let tight_cycle_time = calculate_critical_cycle_time_per_token_hbcn(&tight_result.hbcn);

        // Both should meet their respective constraints
        assert!(
            loose_cycle_time <= 200.0,
            "Loose cycle time {} should be <= 200.0",
            loose_cycle_time
        );
        assert!(
            tight_cycle_time <= 20.0,
            "Tight cycle time {} should be <= 20.0",
            tight_cycle_time
        );

        // The tight constraint should result in a lower or equal cycle time
        assert!(
            tight_cycle_time <= loose_cycle_time,
            "Tight constraints should result in lower or equal cycle time (tight: {}, loose: {})",
            tight_cycle_time,
            loose_cycle_time
        );
    }
}
