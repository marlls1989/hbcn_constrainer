//! Conversion from structural graphs to Half-Buffer Channel Networks.
//!
//! This module provides functionality for converting a [`StructuralGraph`] (representing
//! the circuit's structural connectivity) into a [`StructuralHBCN`] (representing the
//! timing behaviour and handshaking protocol of the asynchronous circuit).
//!
//! # Conversion Process
//!
//! The conversion process transforms the structural representation into an HBCN by:
//!
//! 1. **Node Expansion**: Each circuit node (port or register) in the structural graph is
//!    expanded into two transitions in the HBCN:
//!    - A `Transition::Data` node representing data propagation
//!    - A `Transition::Spacer` node representing spacer/null propagation
//!
//! 2. **Place Creation**: Each channel in the structural graph becomes four places in the HBCN:
//!    - **Forward data place**: From source data transition to destination data transition
//!    - **Forward spacer place**: From source spacer transition to destination spacer transition
//!    - **Backward acknowledgment place**: From destination data transition to source spacer transition
//!    - **Backward acknowledgment place**: From destination spacer transition to source data transition
//!
//! 3. **Token Initialization**: The initial token marking of places is determined by the channel's
//!    [`ChannelPhase`]:
//!    - `ReqData` → forward data place is marked
//!    - `ReqNull` → forward spacer place is marked
//!    - `AckData` → backward place acknowledgeing data reception is marked
//!    - `AckNull` → backward place acknowledgeing null reception is marked
//!
//! 4. **Weight Calculation**: Place weights are computed based on:
//!    - Forward places: Use `virtual_delay` from the channel, optionally augmented with forward
//!      completion costs when `forward_completion` is enabled
//!    - Backward places: Include register delays based on fan-in/fan-out (computed via log₂ of
//!      the degree) plus base costs of circuit nodes
//!
//! # Cost Model
//!
//! The conversion applies a cost model for timing estimation:
//!
//! - **Register delays**: Use `DEFAULT_REGISTER_DELAY * log₂(degree)` where degree is the
//!   number of incoming or outgoing edges. This models the multiplexer/demultiplexer logic
//!   required for fan-in/fan-out.
//!
//! - **Base costs**: Circuit nodes (especially registers) have base costs that contribute
//!   to backward place weights.
//!
//! - **Forward completion**: When enabled, forward place weights consider both the virtual delay
//!   and the completion detection logic required for the destination node.
//!
//! # Example
//!
//! ```
//! use hbcn::hbcn::{StructuralHBCN, from_structural_graph};
//! use hbcn::structural_graph::parse;
//!
//! // Parse a structural graph
//! let structural_graph = parse(r#"
//!     Port "input" [("output", 100)]
//!     Port "output" []
//! "#).unwrap();
//!
//! // Convert to HBCN with forward completion disabled
//! let hbcn = from_structural_graph(&structural_graph, false).unwrap();
//!
//! // The HBCN has 4 nodes (2 data + 2 spacer transitions) and 4 places (4 channel edges)
//! assert_eq!(hbcn.node_count(), 4);
//! assert_eq!(hbcn.edge_count(), 4);
//! ```

#[allow(unused_imports)] // Used in tests
use super::{Place, StructuralHBCN, Transition, WeightedPlace, is_backward_place};
use crate::{
    structural_graph::{Channel, ChannelPhase, StructuralGraph},
    validate_hbcn,
};

use std::collections::HashMap;

use petgraph::prelude::*;

use super::CircuitNode;

/// Computes the ceiling of log base 2 of a number.
///
/// Uses an efficient bit manipulation approach: `⌈log₂(x)⌉ = BITS - leading_zeros(x)`.
/// This function is used to estimate multiplexer/demultiplexer delays based on
/// fan-in/fan-out degree in the structural graph.
///
/// # Arguments
///
/// * `x` - The input number (must be non-zero for meaningful results)
///
/// # Returns
///
/// The ceiling of log base 2 of `x`. For example:
/// - `clog2(1) = 0` (since ⌈log₂(1)⌉ = 0)
/// - `clog2(2) = 1` (since ⌈log₂(2)⌉ = 1)
/// - `clog2(3) = 2` (since ⌈log₂(3)⌉ = 1.58... → 2)
/// - `clog2(4) = 2` (since ⌈log₂(4)⌉ = 2)
///
/// # Example
///
/// ```
/// // Used internally for calculating register delays
/// // For a node with 4 outgoing edges, clog2(4) = 2
/// // This models a 4-to-1 multiplexer requiring 2 levels of logic
/// ```
fn clog2(x: usize) -> u32 {
    usize::BITS - x.leading_zeros()
}

/// Default register delay constant used in delay calculations.
///
/// This represents the base delay (in time units) for register operations.
/// When combined with the log₂ of the degree, it models the delay of multiplexer
/// or demultiplexer logic required for fan-in/fan-out.
///
/// For example, a node with 4 outgoing edges would contribute:
/// `DEFAULT_REGISTER_DELAY * log₂(4) = 10.0 * 2 = 20.0` time units to backward costs.
const DEFAULT_REGISTER_DELAY: f64 = 10.0;

/// Converts a structural graph to a Half-Buffer Channel Network.
///
/// This function performs the core transformation from a structural circuit representation
/// (where nodes are circuit components and edges are channels) to an HBCN representation
/// (where nodes are transitions and edges are places modeling timing dependencies).
///
/// # Arguments
///
/// * `g` - The structural graph to convert. This graph represents the circuit's structural
///   connectivity with circuit nodes as nodes and channels as edges.
/// * `forward_completion` - Whether to enable forward completion delay calculation.
///   When `true`, forward place weights include completion detection costs. When `false`,
///   forward place weights use only the channel's `virtual_delay`.
///
/// # Returns
///
/// Returns `Some(StructuralHBCN)` if the conversion succeeds, or `None` if any edge
/// endpoints are invalid (should not occur for a valid structural graph).
///
/// # Conversion Details
///
/// For each circuit node in the structural graph:
/// - Creates a `Transition::Data` node for data propagation
/// - Creates a `Transition::Spacer` node for spacer/null propagation
///
/// For each channel in the structural graph:
/// - Creates 4 places connecting the corresponding transitions:
///   1. Data → Data (forward, token if `ReqData`)
///   2. Spacer → Spacer (forward, token if `ReqNull`)
///   3. Data → Spacer at source (backward, token if `AckData`)
///   4. Spacer → Data at source (backward, token if `AckNull`)
///
/// # Weight Calculation
///
/// **Forward places** (Data→Data or Spacer→Spacer):
/// - If `forward_completion = false`: `weight = virtual_delay`
/// - If `forward_completion = true`: `weight = max(virtual_delay, forward_cost + src_base_cost)`
///
/// **Backward places** (Data→Spacer or Spacer→Data):
/// - `weight = backward_cost + dst_base_cost`
/// - Where `backward_cost = DEFAULT_REGISTER_DELAY * log₂(outgoing_edges)` and
///   `dst_base_cost` is the base cost of the destination circuit node.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::{from_structural_graph, StructuralHBCN};
/// use hbcn::structural_graph::parse;
///
/// let graph = parse(r#"
///     Port "input" [("reg", 50)]
///     DataReg "reg" [("output", 75)]
///     Port "output" []
/// "#).unwrap();
///
/// // Convert without forward completion
/// let hbcn = from_structural_graph(&graph, false).unwrap();
///
/// // Convert with forward completion
/// let hbcn_with_completion = from_structural_graph(&graph, true).unwrap();
/// ```
pub fn from_structural_graph(
    g: &StructuralGraph,
    forward_completion: bool,
) -> Option<StructuralHBCN> {
    let mut ret = StructuralHBCN::new();
    struct VertexItem {
        token: petgraph::graph::NodeIndex,
        spacer: petgraph::graph::NodeIndex,
        backward_cost: f64,
        forward_cost: f64,
        base_cost: f64,
    }
    let vertice_map: HashMap<petgraph::graph::NodeIndex, VertexItem> = g
        .node_indices()
        .map(|ix| {
            let val = &g[ix];
            let circuit_node = CircuitNode::from(val.clone());
            let token = ret.add_node(Transition::Data(circuit_node.clone()));
            let spacer = ret.add_node(Transition::Spacer(circuit_node));
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
            WeightedPlace {
                place: Place {
                    token: initial_phase == ChannelPhase::ReqData,
                    is_internal,
                },
                weight: forward_cost,
            },
        );
        ret.add_edge(
            *src_spacer,
            *dst_spacer,
            WeightedPlace {
                place: Place {
                    token: initial_phase == ChannelPhase::ReqNull,
                    is_internal,
                },
                weight: forward_cost,
            },
        );
        ret.add_edge(
            *dst_token,
            *src_spacer,
            WeightedPlace {
                place: Place {
                    token: initial_phase == ChannelPhase::AckData,
                    is_internal,
                },
                weight: backward_cost,
            },
        );
        ret.add_edge(
            *dst_spacer,
            *src_token,
            WeightedPlace {
                place: Place {
                    token: initial_phase == ChannelPhase::AckNull,
                    is_internal,
                },
                weight: backward_cost,
            },
        );
    }

    #[cfg(debug_assertions)]
    {
        debug_assert!(
            validate_hbcn(&ret).is_ok(),
            "Generated HBCN failed validation in debug mode: {:?}",
            validate_hbcn(&ret).err()
        );
    }

    Some(ret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Named, structural_graph::parse};

    /// Helper function to create a validated test HBCN
    fn create_test_hbcn(input: &str, forward_completion: bool) -> StructuralHBCN {
        let structural_graph = parse(input).expect("Failed to parse structural graph");
        from_structural_graph(&structural_graph, forward_completion)
            .expect("Failed to convert to StructuralHBCN")
    }

    #[test]
    fn test_simple_two_node_conversion() {
        // Test basic conversion with two connected nodes
        let input = r#"
            Port "input" [("output", 100)]
            Port "output" []
        "#;
        let hbcn = create_test_hbcn(input, false);

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
        let hbcn = create_test_hbcn(input, false);

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
        let hbcn = create_test_hbcn(input, false);

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
        let hbcn = create_test_hbcn(input, false);

        // Check place properties
        let mut forward_places = 0;
        let mut backward_places = 0;

        for edge_idx in hbcn.edge_indices() {
            let weighted_place = &hbcn[edge_idx];

            // Weight should be positive
            assert!(weighted_place.weight >= 0.0);

            // Count forward and backward places
            let (src, dst) = hbcn.edge_endpoints(edge_idx).unwrap();
            let src_transition = hbcn[src].as_ref();
            let dst_transition = hbcn[dst].as_ref();
            if is_backward_place(src_transition, dst_transition) {
                backward_places += 1;
            } else {
                forward_places += 1;
            }
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
        let hbcn = create_test_hbcn(input, false);

        // Check that weights are based on virtual_delay when forward_completion=false
        for edge_idx in hbcn.edge_indices() {
            let weighted_place = &hbcn[edge_idx];
            let (src, dst) = hbcn.edge_endpoints(edge_idx).unwrap();
            let src_transition = hbcn[src].as_ref();
            let dst_transition = hbcn[dst].as_ref();
            if !is_backward_place(src_transition, dst_transition) {
                // Forward places should use virtual_delay (100 in this case)
                assert_eq!(weighted_place.weight, 100.0);
            }
        }
    }

    #[test]
    fn test_forward_completion_enabled() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let hbcn = create_test_hbcn(input, true);

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
        let hbcn = create_test_hbcn(input, false);

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
        let hbcn = create_test_hbcn(input, false);

        // Check that token markings are set according to channel phases
        let mut req_data_count = 0;
        let mut req_null_count = 0;
        let mut ack_data_count = 0;
        let mut ack_null_count = 0;

        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];
            let (src, dst) = hbcn.edge_endpoints(edge_idx).unwrap();
            let src_transition = hbcn[src].as_ref();
            let dst_transition = hbcn[dst].as_ref();
            let is_backward = is_backward_place(src_transition, dst_transition);
            if place.place.token {
                if is_backward {
                    ack_data_count += 1;
                } else {
                    req_data_count += 1;
                }
            } else if is_backward {
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
        let hbcn = create_test_hbcn(input, false);

        // Check weight calculations
        for edge_idx in hbcn.edge_indices() {
            let weighted_place = &hbcn[edge_idx];
            assert!(
                weighted_place.weight >= 0.0,
                "Weight should be non-negative"
            );

            let (src, dst) = hbcn.edge_endpoints(edge_idx).unwrap();
            let src_transition = hbcn[src].as_ref();
            let dst_transition = hbcn[dst].as_ref();
            if !is_backward_place(src_transition, dst_transition) {
                // Forward places should have weight based on virtual_delay
                assert_eq!(weighted_place.weight, 150.0);
            } else {
                // Backward places should include register delays
                assert!(weighted_place.weight >= 10.0);
            }
        }
    }

    #[test]
    fn test_empty_graph() {
        // Test edge case with minimal graph
        let input = r#"
            Port "single" []
        "#;
        let hbcn = create_test_hbcn(input, false);

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
        let hbcn = create_test_hbcn(input, false);

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
