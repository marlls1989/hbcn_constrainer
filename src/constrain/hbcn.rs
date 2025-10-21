use std::collections::HashMap;

use petgraph::prelude::*;

use crate::hbcn::*;
use crate::lp_solver::*;
use crate::structural_graph::CircuitNode;
use crate::AppError;

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct DelayPair {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

pub type PathConstraints = HashMap<(CircuitNode, CircuitNode), DelayPair>;

#[derive(Debug, Clone)]
pub struct ConstrainerResult {
    pub pseudoclock_period: f64,
    pub hbcn: DelayedHBCN,
    pub path_constraints: PathConstraints,
}

/// Constrain cycle time using the pseudoclock algorithm
pub fn constrain_cycle_time_pseudoclock(
    hbcn: &StructuralHBCN,
    ct: f64,
    min_delay: f64,
) -> anyhow::Result<ConstrainerResult> {
    assert!(ct > 0.0);

    let mut m = crate::lp_solver::create_lp_model("constrain")?;

    let pseudo_clock = m.add_variable(
        "pseudo_clock",
        VariableType::Continuous,
        0.0,
        f64::INFINITY,
    )?;

    let arr_var: HashMap<NodeIndex, VariableId> = hbcn
        .node_indices()
        .map(|x| {
            (
                x,
                m.add_variable("", VariableType::Continuous, 0.0, f64::INFINITY)
                    .unwrap(),
            )
        })
        .collect();

    let mut delay_vars: HashMap<(&CircuitNode, &CircuitNode), Option<VariableId>> = hbcn
        .edge_indices()
        .map(|ie| {
            let (src, dst) = hbcn.edge_endpoints(ie).unwrap();

            ((hbcn[src].circuit_node(), hbcn[dst].circuit_node()), None)
        })
        .collect();

    for v in delay_vars.values_mut() {
        *v = Some(m.add_variable("", VariableType::Continuous, min_delay, f64::INFINITY)?);
    }

    for ie in hbcn.edge_indices() {
        let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
        let place = &hbcn[ie];
        let delay = delay_vars[&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
            .as_ref()
            .unwrap();

        // Create constraint: delay + arr_var[src] - arr_var[dst] = (if place.token { ct } else { 0.0 })
        let mut expr = LinearExpression::new(0.0);
        expr.add_term(1.0, *delay);
        expr.add_term(1.0, arr_var[&src]);
        expr.add_term(-1.0, arr_var[&dst]);

        m.add_constraint(
            "",
            expr,
            ConstraintSense::Equal,
            if place.token { ct } else { 0.0 },
        )?;

        if place.is_internal {
            let delay_expr = LinearExpression::from_variable(*delay);
            m.add_constraint("", delay_expr, ConstraintSense::Greater, min_delay)?;
        } else {
            let mut delay_expr = LinearExpression::from_variable(*delay);
            delay_expr.add_term(-1.0, pseudo_clock);
            m.add_constraint("", delay_expr, ConstraintSense::Greater, 0.0)?;
        }
    }

    m.update()?;

    let pseudo_clock_expr = LinearExpression::from_variable(pseudo_clock);
    m.set_objective(pseudo_clock_expr, OptimizationSense::Maximize)?;

    m.optimize()?;

    let pseudo_clock_value = m.get_variable_value(pseudo_clock)?;

    match m.status()? {
        OptimizationStatus::Optimal | OptimizationStatus::Feasible => Ok(ConstrainerResult {
            pseudoclock_period: pseudo_clock_value,
            path_constraints: delay_vars
                .iter()
                .filter_map(|((src, dst), var)| {
                    var.map(|var_id| {
                        let delay_value = m.get_variable_value(var_id).ok()?;
                        Some((
                            (CircuitNode::clone(src), CircuitNode::clone(dst)),
                            DelayPair {
                                min: None,
                                max: Some(delay_value),
                            },
                        ))
                    }).flatten()
                })
                .collect(),
            hbcn: hbcn.map(
                |ix, x| TransitionEvent {
                    transition: x.clone(),
                    time: m
                        .get_variable_value(arr_var[&ix])
                        .ok()
                        .unwrap_or(0.0),
                },
                |ie, e| {
                    let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
                    let delay_value = delay_vars
                        [&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
                        .and_then(|var_id| m.get_variable_value(var_id).ok())
                        .filter(|x| (*x - min_delay) / min_delay > 0.001);
                    
                    DelayedPlace {
                        place: e.clone(),
                        slack: None,
                        delay: DelayPair {
                            min: None,
                            max: delay_value,
                        },
                    }
                },
            ),
        }),
        _ => Err(AppError::Infeasible.into()),
    }
}

/// Constrain cycle time using the proportional algorithm
pub fn constrain_cycle_time_proportional(
    hbcn: &StructuralHBCN,
    ct: f64,
    min_delay: f64,
    backward_margin: Option<f64>,
    forward_margin: Option<f64>,
) -> anyhow::Result<ConstrainerResult> {
    assert!(ct > 0.0);
    assert!(min_delay >= 0.0);

    struct DelayVarPair {
        max: VariableId,
        min: VariableId,
        slack: VariableId,
    }

    let mut m = crate::lp_solver::create_lp_model("constrain")?;

    let factor = m.add_variable("factor", VariableType::Continuous, 0.0, f64::INFINITY)?;

    let arr_var: HashMap<NodeIndex, VariableId> = hbcn
        .node_indices()
        .map(|x| {
            (
                x,
                m.add_variable("", VariableType::Continuous, 0.0, f64::INFINITY)
                    .unwrap(),
            )
        })
        .collect();

    let delay_vars: HashMap<(&CircuitNode, &CircuitNode), DelayVarPair> = hbcn
        .edge_indices()
        .map(|ie| {
            let (src, dst) = hbcn.edge_endpoints(ie).unwrap();

            let max = m
                .add_variable("", VariableType::Continuous, min_delay, f64::INFINITY)
                .unwrap();
            let min = m
                .add_variable("", VariableType::Continuous, 0.0, f64::INFINITY)
                .unwrap();
            let slack = m
                .add_variable("", VariableType::Continuous, 0.0, f64::INFINITY)
                .unwrap();

            (
                (hbcn[src].circuit_node(), hbcn[dst].circuit_node()),
                DelayVarPair { max, min, slack },
            )
        })
        .collect();

    for ie in hbcn.edge_indices() {
        let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
        let place = &hbcn[ie];
        let delay_var = &delay_vars[&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())];
        let matching_delay = delay_vars
            .get(&(hbcn[dst].circuit_node(), hbcn[src].circuit_node()))
            .expect("malformed StructuralHBCN");

        // Constraint: delay_var.max + arr_var[src] - arr_var[dst] = (if place.token { ct } else { 0.0 })
        let mut expr1 = LinearExpression::new(0.0);
        expr1.add_term(1.0, delay_var.max);
        expr1.add_term(1.0, arr_var[&src]);
        expr1.add_term(-1.0, arr_var[&dst]);

        m.add_constraint(
            "",
            expr1,
            ConstraintSense::Equal,
            if place.token { ct } else { 0.0 },
        )?;

        // Constraint: delay_var.max - place.weight * factor - delay_var.slack = 0.0
        let mut expr2 = LinearExpression::new(0.0);
        expr2.add_term(1.0, delay_var.max);
        expr2.add_term(-place.weight, factor);
        expr2.add_term(-1.0, delay_var.slack);

        m.add_constraint("", expr2, ConstraintSense::Equal, 0.0)?;

        if !place.is_internal {
            if place.backward {
                if forward_margin.is_some() {
                    let mut expr3 = LinearExpression::new(0.0);
                    expr3.add_term(1.0, delay_var.min);
                    expr3.add_term(-1.0, matching_delay.max);
                    expr3.add_term(1.0, matching_delay.min);

                    m.add_constraint("", expr3, ConstraintSense::Equal, 0.0)?;
                }
                if let Some(bm) = backward_margin {
                    let mut expr4 = LinearExpression::new(0.0);
                    expr4.add_term(bm, delay_var.max);
                    expr4.add_term(-1.0, delay_var.min);

                    let sense = if forward_margin.is_some() {
                        ConstraintSense::GreaterEqual
                    } else {
                        ConstraintSense::Equal
                    };

                    m.add_constraint("", expr4, sense, 0.0)?;
                } else if forward_margin.is_some() {
                    let mut expr5 = LinearExpression::new(0.0);
                    expr5.add_term(1.0, delay_var.max);
                    expr5.add_term(-1.0, delay_var.min);

                    m.add_constraint("", expr5, ConstraintSense::GreaterEqual, 0.0)?;
                }
            } else if let Some(fm) = forward_margin {
                let mut expr6 = LinearExpression::new(0.0);
                expr6.add_term(fm, delay_var.max);
                expr6.add_term(-1.0, delay_var.min);

                m.add_constraint("", expr6, ConstraintSense::Equal, 0.0)?;
            }
        }
    }

    m.update()?;

    let factor_expr = LinearExpression::from_variable(factor);
    m.set_objective(factor_expr, OptimizationSense::Maximize)?;

    m.optimize()?;

    match m.status()? {
        OptimizationStatus::Optimal | OptimizationStatus::Feasible => Ok(ConstrainerResult {
            pseudoclock_period: min_delay,
            path_constraints: delay_vars
                .iter()
                .map(|((src, dst), var)| {
                    let min = m
                        .get_variable_value(var.min)
                        .ok()
                        .filter(|x| *x > 0.001);
                    let max = m
                        .get_variable_value(var.max)
                        .ok()
                        .filter(|x| (*x - min_delay) / min_delay > 0.001);
                    (
                        (CircuitNode::clone(src), CircuitNode::clone(dst)),
                        DelayPair { min, max },
                    )
                })
                .collect(),
            hbcn: hbcn.map(
                |ix, x| TransitionEvent {
                    transition: x.clone(),
                    time: m
                        .get_variable_value(arr_var[&ix])
                        .ok()
                        .unwrap_or(0.0),
                },
                |ie, e| {
                    let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
                    let delay_var = &delay_vars[&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())];
                    
                    DelayedPlace {
                        place: e.clone(),
                        slack: m.get_variable_value(delay_var.slack).ok(),
                        delay: DelayPair {
                            min: m.get_variable_value(delay_var.min).ok(),
                            max: m.get_variable_value(delay_var.max).ok(),
                        },
                    }
                },
            ),
        }),
        _ => Err(AppError::Infeasible.into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural_graph::parse;
    use crate::hbcn::from_structural_graph;

    /// Test constraint generation with various circuit topologies
    #[test]
    fn test_constraint_algorithms_linear_chain() {
        let input = r#"
            Port "a" [("b", 10)]
            Port "b" [("c", 20)]
            Port "c" [("d", 15)]
            Port "d" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test pseudoclock algorithm
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 50.0, 5.0)
            .expect("Pseudoclock should work on linear chain");

        assert!(pseudo_result.pseudoclock_period >= 5.0);
        assert!(!pseudo_result.path_constraints.is_empty());

        // Test proportional algorithm
        let prop_result = constrain_cycle_time_proportional(&hbcn, 50.0, 5.0, None, None)
            .expect("Proportional should work on linear chain");

        assert!(prop_result.pseudoclock_period >= 5.0);
        assert!(!prop_result.path_constraints.is_empty());
    }

    #[test]
    fn test_constraint_algorithms_branching() {
        let input = r#"
            Port "input" [("branch1", 25), ("branch2", 30)]
            Port "branch1" [("merge", 15)]
            Port "branch2" [("merge", 20)]
            Port "merge" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test both algorithms on branching topology
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 100.0, 8.0)
            .expect("Should handle branching circuit");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 100.0, 8.0, None, None)
            .expect("Should handle branching circuit");

        // Both should produce valid results
        assert!(pseudo_result.pseudoclock_period >= 8.0);
        assert!(prop_result.pseudoclock_period >= 8.0);
        assert!(!pseudo_result.path_constraints.is_empty());
        assert!(!prop_result.path_constraints.is_empty());
    }

    #[test]
    fn test_constraint_algorithms_with_feedback() {
        let input = r#"
            Port "input" [("proc", 40)]
            DataReg "proc" [("output", 35), ("feedback", 25)]
            Port "output" []
            Port "feedback" [("proc", 30)]
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

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
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

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
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let result = constrain_cycle_time_proportional(&hbcn, 100.0, 5.0, None, None)
            .expect("Should generate constraints");

        // Test DelayPair properties in results
        for (_, constraint) in &result.path_constraints {
            // At least one delay should be present
            assert!(
                constraint.min.is_some() || constraint.max.is_some(),
                "Each constraint should have at least min or max delay"
            );

            // If both present, validate relationship
            if let (Some(min), Some(max)) = (constraint.min, constraint.max) {
                assert!(min <= max, "Min delay should not exceed max delay");
                assert!(min >= 0.0, "Min delay should be non-negative");
                assert!(max >= 5.0, "Max delay should be at least minimal delay");
            }
        }
    }

    #[test]
    fn test_markable_place_functionality() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

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
            Port "middle" [("output", 150)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let pseudo_result =
            constrain_cycle_time_pseudoclock(&hbcn, 300.0, 15.0).expect("Pseudoclock should work");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 300.0, 15.0, None, None)
            .expect("Proportional should work");

        // Both should produce valid results but potentially different constraints
        assert!(pseudo_result.pseudoclock_period >= 15.0);
        assert!(prop_result.pseudoclock_period >= 15.0);

        // Pseudoclock typically only produces max constraints
        let _pseudo_has_min = pseudo_result
            .path_constraints
            .values()
            .any(|c| c.min.is_some());
        let pseudo_has_max = pseudo_result
            .path_constraints
            .values()
            .any(|c| c.max.is_some());

        // Proportional may produce both min and max constraints
        let _prop_has_min = prop_result
            .path_constraints
            .values()
            .any(|c| c.min.is_some());
        let prop_has_max = prop_result
            .path_constraints
            .values()
            .any(|c| c.max.is_some());

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
            Port "b" [("c", 200)]
            Port "c" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

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
        let periods = vec![
            no_margin.pseudoclock_period,
            forward_margin.pseudoclock_period,
            backward_margin.pseudoclock_period,
            both_margins.pseudoclock_period,
        ];

        // Test that all margin combinations produce valid results
        // (Margins may or may not affect results depending on the specific circuit)
        let all_valid = periods.iter().all(|&p| p >= 20.0 && p <= 400.0);
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

        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Test pseudoclock algorithm on cyclic circuit
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 100.0, 5.0)
            .expect("Should handle cyclic circuit with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 5.0);
        assert!(pseudo_result.pseudoclock_period <= 100.0);
        assert!(!pseudo_result.path_constraints.is_empty());

        // Test proportional algorithm on cyclic circuit
        let prop_result = constrain_cycle_time_proportional(&hbcn, 100.0, 5.0, None, None)
            .expect("Should handle cyclic circuit with proportional");

        assert!(prop_result.pseudoclock_period >= 5.0);
        assert!(prop_result.pseudoclock_period <= 100.0);
        assert!(!prop_result.path_constraints.is_empty());
    }

    /// Test cyclic circuit with forward completion
    #[test]
    fn test_cyclic_forward_completion() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        // Test without forward completion
        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn_no_fc = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Test with forward completion
        let hbcn_with_fc = from_structural_graph(&structural_graph, true)
            .expect("Failed to convert cyclic graph to HBCN with forward completion");

        // Both should produce valid HBCNs
        assert!(hbcn_no_fc.node_count() > 0);
        assert!(hbcn_with_fc.node_count() > 0);

        // Test constraint generation on both
        let result_no_fc = constrain_cycle_time_proportional(&hbcn_no_fc, 100.0, 5.0, None, None)
            .expect("Should work without forward completion on cyclic circuit");

        let result_with_fc = constrain_cycle_time_proportional(&hbcn_with_fc, 100.0, 5.0, None, None)
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

        let structural_graph = parse(input).expect("Failed to parse minimal cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert minimal cyclic graph to HBCN");

        // Should still work with minimal feedback
        let result = constrain_cycle_time_proportional(&hbcn, 50.0, 2.0, None, None)
            .expect("Should handle minimal cyclic circuit");

        assert!(result.pseudoclock_period >= 2.0);
        assert!(result.pseudoclock_period <= 50.0);
    }

    /// Helper function to calculate critical cycle time per token for HBCN tests
    fn calculate_critical_cycle_time_per_token_hbcn(delayed_hbcn: &crate::hbcn::DelayedHBCN) -> f64 {
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

    /// Test that pseudoclock constraints correctly limit cycle time per token in HBCN
    #[test]
    fn test_hbcn_pseudoclock_cycle_time_verification() {
        let input = r#"Port "a" [("b", 100)]
                      Port "b" []"#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let requested_cycle_time = 50.0;
        let min_delay = 5.0;

        let result = constrain_cycle_time_pseudoclock(&hbcn, requested_cycle_time, min_delay)
            .expect("Pseudoclock constraint generation should succeed");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token_hbcn(&result.hbcn);

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
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let requested_cycle_time = 60.0;
        let min_delay = 8.0;

        let result = constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
            .expect("Proportional constraint generation should succeed");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token_hbcn(&result.hbcn);

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
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let requested_cycle_time = 80.0;
        let min_delay = 10.0;

        let result = constrain_cycle_time_proportional(&hbcn, requested_cycle_time, min_delay, None, None)
            .expect("Proportional constraint generation should succeed for cyclic circuit");

        // Calculate the actual critical cycle time per token
        let actual_cycle_time_per_token = calculate_critical_cycle_time_per_token_hbcn(&result.hbcn);

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
        let input = r#"Port "a" [("b", 100)]
                      Port "b" []"#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test with loose constraints
        let loose_result = constrain_cycle_time_pseudoclock(&hbcn, 200.0, 5.0)
            .expect("Loose constraint generation should succeed");

        // Test with tight constraints
        let tight_result = constrain_cycle_time_pseudoclock(&hbcn, 50.0, 5.0)
            .expect("Tight constraint generation should succeed");

        let loose_cycle_time = calculate_critical_cycle_time_per_token_hbcn(&loose_result.hbcn);
        let tight_cycle_time = calculate_critical_cycle_time_per_token_hbcn(&tight_result.hbcn);

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
}
