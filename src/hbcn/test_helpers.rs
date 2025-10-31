//! Test helpers for creating valid HBCN instances for testing.
//!
//! This module provides helper functions to create valid HBCN instances that conform
//! to the HBCN validation rules. All HBCNs created by these functions are guaranteed
//! to pass `validate_hbcn`.

use crate::hbcn::*;
use petgraph::stable_graph::StableGraph;
use string_cache::DefaultAtom;

/// HBCN type alias for test graphs with DelayedPlace edges
pub type TestHBCN = StableGraph<Transition, DelayedPlace>;

/// Create a valid HBCN with a single channel between two circuit nodes.
///
/// For a channel between nodes "a" and "b", this creates all 4 required places:
/// - Data(a) -> Data(b) (forward data)
/// - Spacer(a) -> Spacer(b) (forward spacer)
/// - Data(b) -> Spacer(a) (backward acknowledgment)
/// - Spacer(b) -> Data(a) (backward acknowledgment)
///
/// Exactly one of these 4 places will be marked (have token=true) based on the `token_place` parameter:
/// - 0: Data(a) -> Data(b) is marked
/// - 1: Data(b) -> Spacer(a) is marked
/// - 2: Spacer(a) -> Spacer(b) is marked
/// - 3: Spacer(b) -> Data(a) is marked
///
/// # Arguments
///
/// * `node_a_name` - Name of the first circuit node
/// * `node_b_name` - Name of the second circuit node
/// * `forward_weight` - Weight for forward places (Data->Data and Spacer->Spacer)
/// * `backward_weight` - Weight for backward places (acknowledgment places)
/// * `token_place` - Which of the 4 places should be marked (0-3)
///
/// # Returns
///
/// Returns a valid HBCN that passes `validate_hbcn`.
pub fn create_valid_channel(
    node_a_name: &str,
    node_b_name: &str,
    forward_weight: f64,
    backward_weight: f64,
    token_place: usize,
) -> TestHBCN {
    let mut g: TestHBCN = StableGraph::new();
    
    let node_a = CircuitNode::Port(DefaultAtom::from(node_a_name));
    let node_b = CircuitNode::Port(DefaultAtom::from(node_b_name));
    
    // Create all 4 required transitions
    let data_a = g.add_node(Transition::Data(node_a.clone()));
    let spacer_a = g.add_node(Transition::Spacer(node_a.clone()));
    let data_b = g.add_node(Transition::Data(node_b.clone()));
    let spacer_b = g.add_node(Transition::Spacer(node_b.clone()));
    
    // Create all 4 required places
    // 1. Data(a) -> Data(b) (forward data)
    g.add_edge(
        data_a,
        data_b,
        DelayedPlace {
            place: Place {
                backward: false,
                token: token_place == 0,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: forward_weight,
            },
            slack: None,
        },
    );
    
    // 2. Data(b) -> Spacer(a) (backward acknowledgment)
    g.add_edge(
        data_b,
        spacer_a,
        DelayedPlace {
            place: Place {
                backward: true,
                token: token_place == 1,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: backward_weight,
            },
            slack: None,
        },
    );
    
    // 3. Spacer(a) -> Spacer(b) (forward spacer)
    g.add_edge(
        spacer_a,
        spacer_b,
        DelayedPlace {
            place: Place {
                backward: false,
                token: token_place == 2,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: forward_weight,
            },
            slack: None,
        },
    );
    
    // 4. Spacer(b) -> Data(a) (backward acknowledgment)
    g.add_edge(
        spacer_b,
        data_a,
        DelayedPlace {
            place: Place {
                backward: true,
                token: token_place == 3,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: backward_weight,
            },
            slack: None,
        },
    );
    
    g
}

/// Create a valid two-channel HBCN: a -> b -> c
///
/// Creates two valid channels: (a, b) and (b, c), each with all 4 required places.
pub fn create_valid_two_channel_hbcn(
    node_a_name: &str,
    node_b_name: &str,
    node_c_name: &str,
    forward_weight_ab: f64,
    backward_weight_ab: f64,
    forward_weight_bc: f64,
    backward_weight_bc: f64,
    token_place_ab: usize,
    token_place_bc: usize,
) -> TestHBCN {
    let mut g: TestHBCN = StableGraph::new();
    
    let node_a = CircuitNode::Port(DefaultAtom::from(node_a_name));
    let node_b = CircuitNode::Port(DefaultAtom::from(node_b_name));
    let node_c = CircuitNode::Port(DefaultAtom::from(node_c_name));
    
    // Create all 6 required transitions
    let data_a = g.add_node(Transition::Data(node_a.clone()));
    let spacer_a = g.add_node(Transition::Spacer(node_a.clone()));
    let data_b = g.add_node(Transition::Data(node_b.clone()));
    let spacer_b = g.add_node(Transition::Spacer(node_b.clone()));
    let data_c = g.add_node(Transition::Data(node_c.clone()));
    let spacer_c = g.add_node(Transition::Spacer(node_c.clone()));
    
    // Channel (a, b): all 4 places
    g.add_edge(
        data_a,
        data_b,
        DelayedPlace {
            place: Place {
                backward: false,
                token: token_place_ab == 0,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: forward_weight_ab,
            },
            slack: None,
        },
    );
    g.add_edge(
        data_b,
        spacer_a,
        DelayedPlace {
            place: Place {
                backward: true,
                token: token_place_ab == 1,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: backward_weight_ab,
            },
            slack: None,
        },
    );
    g.add_edge(
        spacer_a,
        spacer_b,
        DelayedPlace {
            place: Place {
                backward: false,
                token: token_place_ab == 2,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: forward_weight_ab,
            },
            slack: None,
        },
    );
    g.add_edge(
        spacer_b,
        data_a,
        DelayedPlace {
            place: Place {
                backward: true,
                token: token_place_ab == 3,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: backward_weight_ab,
            },
            slack: None,
        },
    );
    
    // Channel (b, c): all 4 places
    g.add_edge(
        data_b,
        data_c,
        DelayedPlace {
            place: Place {
                backward: false,
                token: token_place_bc == 0,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: forward_weight_bc,
            },
            slack: None,
        },
    );
    g.add_edge(
        data_c,
        spacer_b,
        DelayedPlace {
            place: Place {
                backward: true,
                token: token_place_bc == 1,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: backward_weight_bc,
            },
            slack: None,
        },
    );
    g.add_edge(
        spacer_b,
        spacer_c,
        DelayedPlace {
            place: Place {
                backward: false,
                token: token_place_bc == 2,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: forward_weight_bc,
            },
            slack: None,
        },
    );
    g.add_edge(
        spacer_c,
        data_b,
        DelayedPlace {
            place: Place {
                backward: true,
                token: token_place_bc == 3,
                is_internal: false,
            },
            delay: DelayPair {
                min: None,
                max: backward_weight_bc,
            },
            slack: None,
        },
    );
    
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_valid_channel() {
        let hbcn = create_valid_channel("a", "b", 10.0, 5.0, 0);
        validate_hbcn(&hbcn).expect("Created HBCN should be valid");
        
        assert_eq!(hbcn.node_count(), 4); // Data(a), Spacer(a), Data(b), Spacer(b)
        assert_eq!(hbcn.edge_count(), 4); // 4 places for one channel
    }

    #[test]
    fn test_create_valid_channel_all_token_places() {
        // Test all 4 token placement options
        for token_place in 0..4 {
            let hbcn = create_valid_channel("x", "y", 10.0, 5.0, token_place);
            validate_hbcn(&hbcn).expect(&format!("HBCN with token_place={} should be valid", token_place));
        }
    }

    #[test]
    fn test_create_valid_two_channel_hbcn() {
        let hbcn = create_valid_two_channel_hbcn("a", "b", "c", 10.0, 5.0, 8.0, 4.0, 0, 2);
        validate_hbcn(&hbcn).expect("Created two-channel HBCN should be valid");
        
        assert_eq!(hbcn.node_count(), 6); // 3 nodes * 2 transitions each
        assert_eq!(hbcn.edge_count(), 8); // 2 channels * 4 places each
    }
}

