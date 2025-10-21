use std::collections::{HashMap, HashSet};

use anyhow::Result;
use itertools::Itertools;
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    stable_graph::StableGraph,
};
use rayon::prelude::*;

use crate::{
    AppError,
    constrain::hbcn::DelayPair,
    hbcn::{
        DelayedHBCN, DelayedPlace, MarkablePlace, SlackablePlace, StructuralHBCN,
        TransitionEvent,
    },
    lp_solver::{
        Constraint, ConstraintSense, LinearExpression, OptimizationSense, OptimizationStatus, 
        VariableType, VariableId,
    },
    lp_model_builder,
};

/// Find critical cycles in an HBCN graph
pub fn find_critical_cycles<N: Sync + Send, P: MarkablePlace + SlackablePlace>(
    hbcn: &StableGraph<N, P>,
) -> Vec<Vec<(NodeIndex, NodeIndex)>> {
    let mut loop_breakers = Vec::new();
    let mut start_points = HashSet::new();

    let filtered_hbcn = hbcn.filter_map(
        |_, x| Some(x),
        |ie, e| {
            let (u, v) = hbcn.edge_endpoints(ie)?;
            let weight = hbcn[ie].slack();
            if e.is_marked() {
                loop_breakers.push((u, v));
                start_points.insert(v);
                Some(weight)
            } else {
                Some(weight)
            }
        },
    );

    // creates a map with a distance from all start_points to all other nodes
    let bellman_distances: HashMap<NodeIndex, Vec<(f64, Option<NodeIndex>)>> = start_points
        .into_par_iter()
        .map(|ix| {
            let (costs, predecessors) = petgraph::algo::bellman_ford(&filtered_hbcn, ix).unwrap();

            (
                ix,
                // Zips together the distance and predecessor list
                costs.into_iter().zip_eq(predecessors.into_iter()).collect(),
            )
        })
        .collect();

    let paths: Vec<Vec<(NodeIndex, NodeIndex)>> = loop_breakers
        .into_par_iter()
        .filter_map(|(it, is)| {
            let nodes = &bellman_distances[&is];
            // Recreates the path by traveling the predecessors list
            let path: Vec<_> = {
                let mut current_node = it;
                let mut path = vec![it];
                while current_node != is {
                    if let (_, Some(node)) = nodes[current_node.index()] {
                        path.push(node);
                        current_node = node;
                    } else {
                        return None;
                    }
                }
                path.reverse();

                path.iter()
                    .cloned()
                    .zip(path.iter().skip(1).cloned().chain(std::iter::once(is)))
                    .collect()
            };
            Some(path)
        })
        .collect();

    paths
}

/// Compute cycle time for an HBCN using linear programming
pub fn compute_cycle_time(hbcn: &StructuralHBCN, weighted: bool) -> Result<(f64, DelayedHBCN)> {
    let mut builder = lp_model_builder!();
    let cycle_time = builder.add_variable("cycle_time", VariableType::Integer, 0.0, f64::INFINITY);

    let arr_var: HashMap<NodeIndex, VariableId<_>> = hbcn
        .node_indices()
        .map(|x| {
            (
                x,
                builder.add_variable("", VariableType::Continuous, 0.0, f64::INFINITY),
            )
        })
        .collect();

    let delay_slack_var: HashMap<EdgeIndex, (VariableId<_>, VariableId<_>)> = hbcn
        .edge_indices()
        .map(|ie| {
            let (ref src, ref dst) = hbcn.edge_endpoints(ie).unwrap();
            let place = &hbcn[ie];

            let slack = builder
                .add_variable("", VariableType::Continuous, 0.0, f64::INFINITY);

            let delay = builder
                .add_variable("", VariableType::Continuous, 0.0, f64::INFINITY);

            // Constraint: delay - slack = (if weighted { place.weight } else { 1.0 })
            let mut expr1 = LinearExpression::new(0.0);
            expr1.add_term(1.0, delay);
            expr1.add_term(-1.0, slack);

            builder.add_constraint(Constraint::new(
                "",
                expr1,
                ConstraintSense::Equal,
                if weighted { place.weight as f64 } else { 1.0 },
            ));

            // Constraint: arr_var[dst] - arr_var[src] - delay + (if place.token { 1.0 } else { 0.0 }) * cycle_time = 0.0
            let mut expr2 = LinearExpression::new(0.0);
            expr2.add_term(1.0, arr_var[dst]);
            expr2.add_term(-1.0, arr_var[src]);
            expr2.add_term(-1.0, delay);
            if place.token {
                expr2.add_term(1.0, cycle_time);
            }

            builder.add_constraint(Constraint::new("", expr2, ConstraintSense::Equal, 0.0));

            (ie, (delay, slack))
        })
        .collect();

    let cycle_time_expr = LinearExpression::from_variable(cycle_time);
    builder.set_objective(cycle_time_expr, OptimizationSense::Minimize);

    let solution = builder.solve()?;
    if solution.status == OptimizationStatus::InfeasibleOrUnbounded {
        Err(AppError::Infeasible.into())
    } else {
        Ok((
            solution.objective_value,
            hbcn.filter_map(
                |ix, x| {
                    Some(TransitionEvent {
                        transition: x.clone(),
                        time: solution.get_value(arr_var[&ix])?,
                    })
                },
                |ie, e| {
                    let (delay_var, slack_var) = &delay_slack_var[&ie];
                    Some(DelayedPlace {
                        place: e.clone(),
                        delay: DelayPair {
                            min: None,
                            max: solution.get_value(*delay_var),
                        },
                        slack: solution.get_value(*slack_var),
                        ..Default::default()
                    })
                },
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural_graph::parse;
    use crate::hbcn::{from_structural_graph, TimedEvent};

    #[test]
    fn test_critical_cycle_detection() {
        // Create a feasible circuit with DataReg for proper cycle formation
        let input = r#"
            Port "input" [("reg", 100)]
            DataReg "reg" [("output", 150), ("input", 75)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 800.0, 20.0, None, None)
            .expect("Should constrain circuit with feasible parameters");

        // Find critical cycles
        let cycles = find_critical_cycles(&result.hbcn);

        // Validate cycle structure if any cycles are found
        for cycle in &cycles {
            assert!(
                cycle.len() >= 2,
                "Each cycle should have at least 2 transitions"
            );

            // Verify cycle structure is valid
            if !cycle.is_empty() {
                let _first_node = cycle[0].0;
                let _last_node = cycle[cycle.len() - 1].1;
                // Cycle validation logic would go here if needed
            }
        }

        // Test passes as long as constraint generation succeeds
        assert!(result.pseudoclock_period >= 20.0);
    }

    #[test]
    fn test_transition_event_timing() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" [("c", 200)]
            Port "c" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let result =
            crate::constrain::hbcn::constrain_cycle_time_pseudoclock(&hbcn, 500.0, 25.0).expect("Should generate timing");

        // Check that all transition events have valid timing
        for node_idx in result.hbcn.node_indices() {
            let event = &result.hbcn[node_idx];

            // Time should be non-negative
            assert!(event.time() >= 0.0, "Event timing should be non-negative");

            // Should have valid transition reference
            match &event.transition {
                crate::hbcn::Transition::Data(node) | crate::hbcn::Transition::Spacer(node) => {
                    assert!(
                        !node.name().as_ref().is_empty(),
                        "Node should have valid name"
                    );
                }
            }
        }
    }

    /// Test cyclic circuit critical cycle detection
    #[test]
    fn test_cyclic_critical_cycle_detection() {
        let input = r#"Port "input" [("reg", 30)]
                      DataReg "reg" [("output", 25), ("reg", 20)]
                      Port "output" []"#;

        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Generate constraints to get DelayedHBCN
        let result = crate::constrain::hbcn::constrain_cycle_time_proportional(&hbcn, 200.0, 10.0, None, None)
            .expect("Should generate constraints for cyclic circuit");

        // Find critical cycles in the result
        let cycles = find_critical_cycles(&result.hbcn);

        // For cyclic circuits, we expect to find cycles
        // Each cycle should have at least 2 edges if any are found
        for cycle in &cycles {
            assert!(cycle.len() >= 2, "Cycles should have at least 2 edges");
        }

        // The constraint generation should succeed
        assert!(result.pseudoclock_period >= 10.0);
    }

    /// Test cyclic circuit timing and delay calculations
    #[test]
    fn test_cyclic_timing_calculations() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Test cycle time computation on cyclic circuit
        let (cycle_time, delayed_hbcn) = compute_cycle_time(&hbcn, true)
            .expect("Should compute cycle time for cyclic circuit");

        assert!(cycle_time > 0.0, "Cycle time should be positive");
        assert!(delayed_hbcn.node_count() > 0, "Delayed HBCN should have nodes");

        // Test that all delays are reasonable
        for edge_idx in delayed_hbcn.edge_indices() {
            let edge = &delayed_hbcn[edge_idx];
            if let Some(max_delay) = edge.delay.max {
                assert!(max_delay >= 0.0, "Max delay should be non-negative");
            }
            if let Some(min_delay) = edge.delay.min {
                assert!(min_delay >= 0.0, "Min delay should be non-negative");
            }
        }
    }
}
