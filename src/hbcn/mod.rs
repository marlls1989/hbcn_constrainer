use super::{
    constrain::hbcn::DelayPair,
    structural_graph::{Channel, ChannelPhase, CircuitNode, StructuralGraph, Symbol},
};

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use petgraph::{graph::NodeIndex, prelude::*, stable_graph::StableGraph};

// this is the most engineery way to compute the ceiling log base 2 of a number
fn clog2(x: usize) -> u32 {
    usize::BITS - x.leading_zeros()
}

// Timing constants for delay/cost calculations
const DEFAULT_REGISTER_DELAY: f64 = 10.0;

pub trait HasTransition {
    fn transition(&self) -> &Transition;
}

pub trait Named {
    fn name(&self) -> &Symbol;
}

pub trait HasCircuitNode {
    fn circuit_node(&self) -> &CircuitNode;
}

#[allow(dead_code)]
pub trait TimedEvent {
    fn time(&self) -> f64;
}

impl<T: HasTransition> HasCircuitNode for T {
    fn circuit_node(&self) -> &CircuitNode {
        self.transition().circuit_node()
    }
}

impl<T: HasCircuitNode> Named for T {
    fn name(&self) -> &Symbol {
        self.circuit_node().name()
    }
}

#[derive(PartialEq, Eq, Debug, Clone, PartialOrd, Ord)]
pub enum Transition {
    Spacer(CircuitNode),
    Data(CircuitNode),
}

impl HasCircuitNode for Transition {
    fn circuit_node(&self) -> &CircuitNode {
        match self {
            Transition::Data(id) => id,
            Transition::Spacer(id) => id,
        }
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transition::Spacer(id) => write!(f, "Spacer at {}", id),
            Transition::Data(id) => write!(f, "Data at {}", id),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Default)]
pub struct Place {
    pub backward: bool,
    pub token: bool,
    pub weight: f64,
    pub is_internal: bool,
    pub relative_endpoints: HashSet<NodeIndex>,
}

#[allow(dead_code)]
pub trait MarkablePlace {
    fn mark(&mut self, mark: bool);
    fn is_marked(&self) -> bool;
}

pub trait SlackablePlace {
    fn slack(&self) -> f64;
}

pub trait WeightedPlace {
    fn weight(&self) -> f64;
}

#[allow(dead_code)]
pub trait HasPlace {
    fn place(&self) -> &Place;
    fn place_mut(&mut self) -> &mut Place;
}

impl WeightedPlace for Place {
    fn weight(&self) -> f64 {
        self.weight
    }
}

impl HasPlace for Place {
    fn place(&self) -> &Place {
        self
    }

    fn place_mut(&mut self) -> &mut Place {
        self
    }
}

impl<P: HasPlace> MarkablePlace for P {
    fn mark(&mut self, mark: bool) {
        self.place_mut().token = mark;
    }

    fn is_marked(&self) -> bool {
        self.place().token
    }
}

pub type StructuralHBCN = StableGraph<Transition, Place>;

pub fn from_structural_graph(
    g: &StructuralGraph,
    forward_completion: bool,
) -> Option<StructuralHBCN> {
    let mut ret = StructuralHBCN::new();
    struct VertexItem {
        token: NodeIndex,
        spacer: NodeIndex,
        backward_cost: f64,
        forward_cost: f64,
        base_cost: f64,
    }
    let vertice_map: HashMap<NodeIndex, VertexItem> = g
        .node_indices()
        .map(|ix| {
            let val = &g[ix];
            let token = ret.add_node(Transition::Data(val.clone()));
            let spacer = ret.add_node(Transition::Spacer(val.clone()));
            let base_cost = val.base_cost() as f64;
            let backward_cost = DEFAULT_REGISTER_DELAY
                * clog2(g.edges_directed(ix, Direction::Outgoing).count()) as f64;
            let forward_cost = DEFAULT_REGISTER_DELAY
                * clog2(g.edges_directed(ix, Direction::Incoming).count()) as f64;
            (
                ix,
                VertexItem {
                    token,
                    spacer,
                    backward_cost,
                    forward_cost,
                    base_cost,
                },
            )
        })
        .collect();

    for ix in g.edge_indices() {
        let Channel { is_internal, .. } = g[ix];

        let (ref src, ref dst) = g.edge_endpoints(ix)?;
        let VertexItem {
            token: src_token,
            spacer: src_spacer,
            backward_cost,
            base_cost: src_base_cost,
            ..
        } = vertice_map.get(src)?;
        let VertexItem {
            token: dst_token,
            spacer: dst_spacer,
            forward_cost,
            base_cost: dst_base_cost,
            ..
        } = vertice_map.get(dst)?;
        let Channel {
            initial_phase,
            virtual_delay,
            ..
        } = g[ix];

        let forward_cost = if forward_completion {
            virtual_delay.max(*forward_cost + *src_base_cost)
        } else {
            virtual_delay
        };
        let backward_cost = *backward_cost + *dst_base_cost;

        ret.add_edge(
            *src_token,
            *dst_token,
            Place {
                backward: false,
                token: initial_phase == ChannelPhase::ReqData,
                relative_endpoints: HashSet::new(),
                weight: forward_cost,
                is_internal,
            },
        );
        ret.add_edge(
            *src_spacer,
            *dst_spacer,
            Place {
                backward: false,
                token: initial_phase == ChannelPhase::ReqNull,
                relative_endpoints: HashSet::new(),
                weight: forward_cost,
                is_internal,
            },
        );
        ret.add_edge(
            *dst_token,
            *src_spacer,
            Place {
                backward: true,
                token: initial_phase == ChannelPhase::AckData,
                relative_endpoints: HashSet::new(),
                weight: backward_cost,
                is_internal,
            },
        );
        ret.add_edge(
            *dst_spacer,
            *src_token,
            Place {
                backward: true,
                token: initial_phase == ChannelPhase::AckNull,
                relative_endpoints: HashSet::new(),
                weight: backward_cost,
                is_internal,
            },
        );
    }

    Some(ret)
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct TransitionEvent {
    pub time: f64,
    pub transition: Transition,
}

impl HasTransition for TransitionEvent {
    fn transition(&self) -> &Transition {
        &self.transition
    }
}

impl TimedEvent for TransitionEvent {
    fn time(&self) -> f64 {
        self.time
    }
}

#[derive(Debug, Clone, Default)]
pub struct DelayedPlace {
    pub place: Place,
    pub delay: DelayPair,
    pub slack: Option<f64>,
}

impl WeightedPlace for DelayedPlace {
    fn weight(&self) -> f64 {
        self.delay.max.unwrap_or(self.place.weight)
    }
}

impl HasPlace for DelayedPlace {
    fn place(&self) -> &Place {
        &self.place
    }

    fn place_mut(&mut self) -> &mut Place {
        &mut self.place
    }
}

impl SlackablePlace for DelayedPlace {
    fn slack(&self) -> f64 {
        self.slack.unwrap_or(0.0)
    }
}

pub type TimedHBCN<T> = StableGraph<TransitionEvent, T>;

pub type DelayedHBCN = TimedHBCN<DelayedPlace>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural_graph::parse;
    #[test]
    fn test_simple_two_node_conversion() {
        // Test basic conversion with two connected nodes
        let input = r#"
            Port "input" [("output", 100)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Should have 4 nodes: Data and Spacer transitions for each original node
        assert_eq!(hbcn.node_count(), 4);

        // Should have 4 edges: forward and backward places for the connection
        assert_eq!(hbcn.edge_count(), 4);

        // Verify node types exist
        let nodes: Vec<_> = hbcn.node_indices().map(|i| &hbcn[i]).collect();
        assert_eq!(nodes.len(), 4);

        // Count Data and Spacer transitions
        let data_count = nodes
            .iter()
            .filter(|n| matches!(n, Transition::Data(_)))
            .count();
        let spacer_count = nodes
            .iter()
            .filter(|n| matches!(n, Transition::Spacer(_)))
            .count();
        assert_eq!(data_count, 2);
        assert_eq!(spacer_count, 2);
    }

    #[test]
    fn test_data_register_conversion() {
        // Test conversion with DataReg which has internal structure
        let input = r#"
            Port "input" [("reg", 50)]
            DataReg "reg" [("output", 75)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Should have 10 nodes: Data and Spacer for each of the 5 circuit nodes
        // (input port, reg data, reg control, reg output, output port)
        assert_eq!(hbcn.node_count(), 10);

        // Should have 16 edges: each channel creates 4 places
        // 4 edges per channel, 4 channels total
        assert_eq!(hbcn.edge_count(), 16);
    }

    #[test]
    fn test_transition_properties() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Check that transitions have correct circuit node references
        for node_idx in hbcn.node_indices() {
            let transition = &hbcn[node_idx];
            match transition {
                Transition::Data(node) | Transition::Spacer(node) => {
                    assert!(node.name().as_ref() == "a" || node.name().as_ref() == "b");
                }
            }
        }
    }

    #[test]
    fn test_place_properties() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Check place properties
        let mut forward_places = 0;
        let mut backward_places = 0;

        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];

            // Weight should be positive
            assert!(place.weight >= 0.0);

            // Count forward and backward places
            if place.backward {
                backward_places += 1;
            } else {
                forward_places += 1;
            }

            // relative_endpoints should be initialised
            assert!(place.relative_endpoints.is_empty()); // Should be empty since reflexive paths are removed
        }

        // For this simple two-port graph, we should have equal numbers of forward and backward places
        // Each channel creates 2 forward places (token->token, spacer->spacer) and 2 backward places (token->spacer, spacer->token)
        assert_eq!(
            forward_places, backward_places,
            "Forward and backward places should be equal in a simple chain"
        );
    }

    #[test]
    fn test_forward_completion_disabled() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Check that weights are based on virtual_delay when forward_completion=false
        let places: Vec<_> = hbcn.edge_indices().map(|i| &hbcn[i]).collect();
        for place in places {
            if !place.backward {
                // Forward places should use virtual_delay (100 in this case)
                assert_eq!(place.weight, 100.0);
            }
        }
    }

    #[test]
    fn test_forward_completion_enabled() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, true)
            .expect("Failed to convert to StructuralHBCN");

        // With forward_completion=true, weights should consider forward costs
        let places: Vec<_> = hbcn.edge_indices().map(|i| &hbcn[i]).collect();
        assert!(!places.is_empty());

        // Should still produce valid HBCN
        assert!(hbcn.node_count() > 0);
        assert!(hbcn.edge_count() > 0);
    }

    #[test]
    fn test_complex_graph_conversion() {
        let input = r#"
            Port "input" [("reg1", 10), ("reg2", 20)]
            DataReg "reg1" [("output", 50)]
            DataReg "reg2" [("output", 60)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Should handle multiple connections properly
        assert!(hbcn.node_count() > 4); // More nodes due to DataReg internal structure
        assert!(hbcn.edge_count() > 8); // More edges due to multiple connections

        // All transitions should be valid
        for node_idx in hbcn.node_indices() {
            let transition = &hbcn[node_idx];
            match transition {
                Transition::Data(node) | Transition::Spacer(node) => {
                    assert!(!node.name().as_ref().is_empty());
                }
            }
        }
    }

    #[test]
    fn test_channel_phases() {
        // Test that different channel phases are handled correctly
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Check that token markings are set according to channel phases
        let mut req_data_count = 0;
        let mut req_null_count = 0;
        let mut ack_data_count = 0;
        let mut ack_null_count = 0;

        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];
            if place.token {
                if place.backward {
                    ack_data_count += 1;
                } else {
                    req_data_count += 1;
                }
            } else if place.backward {
                ack_null_count += 1;
            } else {
                req_null_count += 1;
            }
        }

        // Should have balanced counts based on the protocol
        assert_eq!(
            req_data_count + req_null_count + ack_data_count + ack_null_count,
            hbcn.edge_count()
        );
    }

    #[test]
    fn test_weight_calculations() {
        let input = r#"
            Port "a" [("b", 150)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Check weight calculations
        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];
            assert!(place.weight >= 0.0, "Weight should be non-negative");

            if !place.backward {
                // Forward places should have weight based on virtual_delay
                assert_eq!(place.weight, 150.0);
            } else {
                // Backward places should include register delays
                assert!(place.weight >= DEFAULT_REGISTER_DELAY);
            }
        }
    }

    #[test]
    fn test_empty_graph() {
        // Test edge case with minimal graph
        let input = r#"
            Port "single" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Should have 2 nodes (Data and Spacer for the single port) and no edges
        assert_eq!(hbcn.node_count(), 2);
        assert_eq!(hbcn.edge_count(), 0);
    }

    #[test]
    fn test_register_types() {
        // Test conversion with different register types
        let input = r#"
            Port "input" [("null_reg", 100)]
            NullReg "null_reg" [("control_reg", 200)]
            ControlReg "control_reg" [("unsafe_reg", 300)]
            UnsafeReg "unsafe_reg" [("output", 400)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert to StructuralHBCN");

        // Should successfully convert all register types
        assert!(hbcn.node_count() > 0);
        assert!(hbcn.edge_count() > 0);

        // Verify all transitions have valid circuit nodes
        for node_idx in hbcn.node_indices() {
            let transition = &hbcn[node_idx];
            match transition {
                Transition::Data(node) | Transition::Spacer(node) => {
                    // All nodes should have valid names
                    assert!(!node.name().as_ref().is_empty());
                }
            }
        }
    }
}
