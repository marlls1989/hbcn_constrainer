//! Half-Buffer Channel Network (HBCN) representation for asynchronous circuit timing analysis.
//!
//! This module provides the core data structures and algorithms for representing and analyzing
//! Half-Buffer Channel Networks, which model the timing behavior of asynchronous digital circuits.
//!
//! # Overview
//!
//! An HBCN is a directed graph where:
//! - **Nodes** represent circuit transitions (data or spacer transitions at circuit nodes)
//! - **Edges** represent places that model timing dependencies between transitions
//!
//! The HBCN model is central to timing analysis in asynchronous circuits because it captures the
//! handshaking protocol behavior inherent in half-buffer channel networks, where data flows
//! through channels with explicit acknowledgment signaling.
//!
//! # Core Concepts
//!
//! ## Transitions
//!
//! A [`Transition`] represents an event at a circuit node and can be either:
//! - **Data**: A data transition carrying information
//! - **Spacer**: A spacer/null transition that enables the next data phase
//!
//! Each transition is associated with a [`CircuitNode`] that identifies the physical location
//! in the circuit where the transition occurs.
//!
//! ## Places
//!
//! A [`Place`] represents timing dependencies between transitions:
//! - **Forward places**: Connect transitions of the same type (Data→Data or Spacer→Spacer), modeling forward data flow timing
//! - **Backward places**: Connect transitions of different types (Data→Spacer or Spacer→Data), modeling acknowledgment/return path timing
//! - **Token marking** (`token`): Indicates whether a place initially contains a token
//!
//! The direction of a place can be determined using [`is_backward_place`] based on the transitions it connects.
//!
//! ## Graph Types
//!
//! The module defines two main HBCN graph types:
//!
//! - **[`StructuralHBCN`]**: Initial HBCN structure created from a structural graph,
//!   with `Transition` nodes and `Place` edges. This represents the circuit structure before
//!   timing analysis.
//!
//! - **[`SolvedHBCN`]**: HBCN after timing analysis, with `TransitionEvent` nodes (transitions
//!   with timing information) and `DelayedPlace` edges (places with computed delay constraints
//!   and slack values). This represents the circuit with resolved timing constraints.
//!
//! # Usage Example
//!
//! ```no_run
//! use hbcn::hbcn::*;
//! use hbcn::structural_graph::parse;
//!
//! // Parse a structural graph from input
//! let structural_graph = parse(r#"
//!     Port "input" [("output", 100)]
//!     Port "output" []
//! "#).unwrap();
//!
//! // Convert to HBCN
//! let hbcn = from_structural_graph(&structural_graph, false).unwrap();
//!
//! // The HBCN can now be used for timing analysis and constraint generation
//! ```
//!
//! # Traits
//!
//! The module provides several trait abstractions for working with HBCN components:
//!
//! - **[`Named`]**: Types that have a name (derived from circuit nodes)
//! - **[`TimedEvent`]**: Types that have a time value
//! - **[`MarkablePlace`]**: Places that can be marked/unmarked (token state)
//! - **[`HasWeight`]**: Trait for places that have a weight/delay
//! - **[`SlackablePlace`]**: Places that have computed slack values
//! - **`AsRef<Place>`** and **`AsMut<Place>`**: Types that can provide references to a place
//!
//! Types that can be converted to `Transition` implement `Into<Transition>`, and types that can
//! be converted to `CircuitNode` implement `Into<CircuitNode>`. This enables generic algorithms
//! that work across different HBCN representations.
//!
//! # Re-exported Types
//!
//! This module also provides simplified versions of types for use with HBCN:
//!
//! - **[`CircuitNode`]**: Simplified circuit node representation (without cost field)
//! - **[`DelayPair`]**: Min/max delay constraint representation used in timing analysis

pub mod parser;
pub mod serialisation;
pub mod structural_graph;
#[cfg(test)]
pub mod test_helpers;
pub use structural_graph::from_structural_graph;

use crate::Symbol;
use crate::structural_graph::CircuitNode as StructuralCircuitNode;
use anyhow::{Result, bail};
use petgraph::graph::{EdgeIndex, NodeIndex};
use petgraph::stable_graph::StableGraph;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Trait for types that have a name.
///
/// Implemented automatically for types that implement `AsRef<CircuitNode>`,
/// since circuit nodes provide names.
pub trait Named {
    /// Returns a reference to the name of this element.
    fn name(&self) -> &Symbol;
}

/// Represents a delay pair with optional minimum and mandatory maximum delay constraints.
///
/// This type is used throughout the HBCN constraint generation process to specify
/// timing requirements on paths between circuit nodes.
///
/// # Fields
///
/// - `min`: Optional minimum delay constraint (in time units)
/// - `max`: Maximum delay constraint (in time units)
///
/// # Example
///
/// ```
/// use hbcn::hbcn::DelayPair;
///
/// // Path with both min and max delays
/// let constraint = DelayPair {
///     min: Some(1.0),
///     max: 8.5,
/// };
///
/// // Path with only max delay
/// let max_only = DelayPair {
///     min: None,
///     max: 10.0,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct DelayPair {
    /// Optional minimum delay constraint.
    pub min: Option<f64>,
    /// Maximum delay constraint.
    pub max: f64,
}

impl DelayPair {
    /// Create a new delay pair with the specified min and max values.
    ///
    /// `min` is optional (can be `None`), while `max` is required as a concrete `f64` value.
    pub fn new(min: Option<f64>, max: f64) -> Self {
        Self { min, max }
    }
}

/// Simplified representation of a circuit node for HBCN operations.
///
/// This is a simplified version of the structural graph's `CircuitNode` that removes
/// the cost field (which is only needed during structural graph construction). This
/// type is used throughout HBCN analysis and constraint generation.
///
/// # Variants
///
/// - **`Port(Symbol)`**: An external interface port (input or output)
/// - **`Register { name: Symbol }`**: A register component
///
/// # Conversion
///
/// This type can be created from a structural graph `CircuitNode` using the
/// `From` trait implementation, which discards the cost information.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::CircuitNode;
/// use string_cache::DefaultAtom;
///
/// let port = CircuitNode::Port(DefaultAtom::from("input"));
/// let register = CircuitNode::Register(DefaultAtom::from("reg1"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CircuitNode {
    /// External interface port.
    Port(Symbol),
    /// Register component.
    Register(Symbol),
}

impl From<StructuralCircuitNode> for CircuitNode {
    fn from(node: StructuralCircuitNode) -> Self {
        match node {
            StructuralCircuitNode::Port(name) => CircuitNode::Port(name),
            StructuralCircuitNode::Register { name, .. } => CircuitNode::Register(name),
        }
    }
}

impl AsRef<CircuitNode> for CircuitNode {
    fn as_ref(&self) -> &CircuitNode {
        self
    }
}

impl fmt::Display for CircuitNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitNode::Port(name) => write!(f, "Port \"{}\"", name),
            CircuitNode::Register(name) => write!(f, "Register \"{}\"", name),
        }
    }
}

/// Trait for types that have an associated time value.
pub trait TimedEvent {
    /// Returns the time value associated with this event.
    fn time(&self) -> f64;
}

impl Named for CircuitNode {
    fn name(&self) -> &Symbol {
        match self {
            CircuitNode::Port(name) => name,
            CircuitNode::Register(name) => name,
        }
    }
}

impl Named for Transition {
    fn name(&self) -> &Symbol {
        AsRef::<CircuitNode>::as_ref(self).name()
    }
}

impl Named for TransitionEvent {
    fn name(&self) -> &Symbol {
        AsRef::<CircuitNode>::as_ref(self).name()
    }
}

/// Represents a transition event at a circuit node in the HBCN.
///
/// Transitions are the fundamental events in half-buffer channel networks. They represent
/// either data propagation or spacer/null propagation through a circuit node. The alternation
/// between data and spacer transitions ensures proper handshaking protocol behavior.
///
/// # Variants
///
/// - **`Data`**: A data transition, representing the transmission of actual data through
///   the circuit node. This corresponds to the data phase of the handshaking protocol.
///
/// - **`Spacer`**: A spacer (null) transition, representing the return-to-zero or null
///   phase that prepares the channel for the next data transmission. This corresponds to
///   the acknowledgment/return phase of the handshaking protocol.
///
/// Each variant contains a [`CircuitNode`] that identifies the physical location in the
/// circuit where this transition occurs.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::{Transition, CircuitNode};
/// use string_cache::DefaultAtom;
///
/// let node = CircuitNode::Port(DefaultAtom::from("input"));
///
/// let data_transition = Transition::Data(node.clone());
/// let spacer_transition = Transition::Spacer(node);
/// ```
#[derive(PartialEq, Eq, Debug, Clone, PartialOrd, Ord)]
pub enum Transition {
    /// Spacer/null transition at a circuit node.
    Spacer(CircuitNode),
    /// Data transition at a circuit node.
    Data(CircuitNode),
}

impl AsRef<CircuitNode> for Transition {
    fn as_ref(&self) -> &CircuitNode {
        match self {
            Transition::Data(id) => id,
            Transition::Spacer(id) => id,
        }
    }
}

impl AsRef<Transition> for Transition {
    fn as_ref(&self) -> &Transition {
        self
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

/// Determines if a place is backward based on its source and destination transitions.
///
/// A place is backward if it connects transitions of different types (Data to Spacer or Spacer to Data).
/// A place is forward if it connects transitions of the same type (Data to Data or Spacer to Spacer).
///
/// # Arguments
///
/// * `src` - The source transition
/// * `dst` - The destination transition
///
/// # Returns
///
/// `true` if the place is backward (connects different transition types), `false` if forward.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::{Transition, CircuitNode, is_backward_place};
/// use string_cache::DefaultAtom;
///
/// let data_a = Transition::Data(CircuitNode::Port(DefaultAtom::from("a")));
/// let data_b = Transition::Data(CircuitNode::Port(DefaultAtom::from("b")));
/// let spacer_a = Transition::Spacer(CircuitNode::Port(DefaultAtom::from("a")));
///
/// // Forward place: Data to Data
/// assert!(!is_backward_place(&data_a, &data_b));
///
/// // Backward place: Data to Spacer
/// assert!(is_backward_place(&data_b, &spacer_a));
/// ```
pub fn is_backward_place(src: &Transition, dst: &Transition) -> bool {
    matches!(
        (src, dst),
        (Transition::Data(_), Transition::Spacer(_)) | (Transition::Spacer(_), Transition::Data(_))
    )
}

/// Represents a place (edge) in the HBCN graph.
///
/// Places model timing dependencies between transitions in the HBCN. They represent
/// the channels and synchronization points in the asynchronous circuit, capturing the
/// timing relationships needed for correct handshaking behavior.
///
/// The direction (forward or backward) of a place can be determined from the transitions
/// it connects using [`is_backward_place`]. Forward places connect transitions of the
/// same type (Data to Data or Spacer to Spacer), while backward places connect transitions
/// of different types (Data to Spacer or Spacer to Data).
///
/// # Fields
///
/// - **`token`**: Whether this place initially contains a token. Tokens represent the
///   initial state of the place in the handshaking protocol and affect the initial marking
///   of the HBCN graph.
///
/// - **`is_internal`**: Whether this place represents an internal connection (between
///   internal circuit components) versus an external channel.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::{Place, Transition, CircuitNode, is_backward_place};
/// use string_cache::DefaultAtom;
///
/// let place = Place {
///     token: true,
///     is_internal: false,
/// };
///
/// // Determine if place is backward from transitions
/// let src = Transition::Data(CircuitNode::Port(DefaultAtom::from("a")));
/// let dst = Transition::Spacer(CircuitNode::Port(DefaultAtom::from("b")));
/// let is_backward = is_backward_place(&src, &dst);
/// ```
#[derive(PartialEq, Debug, Clone, Default)]
pub struct Place {
    /// Whether this place initially contains a token.
    pub token: bool,
    /// Whether this place represents an internal connection.
    pub is_internal: bool,
}

/// Trait for places that can be marked or unmarked (token state).
pub trait MarkablePlace {
    /// Mark or unmark this place (set token state).
    fn mark(&mut self, mark: bool);
    /// Check if this place is marked (has a token).
    fn is_marked(&self) -> bool;
}

impl<P: AsRef<Place> + AsMut<Place>> MarkablePlace for P {
    fn mark(&mut self, mark: bool) {
        self.as_mut().token = mark;
    }

    fn is_marked(&self) -> bool {
        self.as_ref().token
    }
}

/// Trait for places that have a computed slack value.
///
/// Slack represents the difference between required arrival time and actual arrival time,
/// indicating how much timing margin exists for this place.
pub trait SlackablePlace {
    /// Returns the slack value for this place.
    fn slack(&self) -> f64;
}

/// Trait for places that have a weight/delay value.
pub trait HasWeight {
    /// Returns the weight (delay/cost) of this place.
    fn weight(&self) -> f64;
}

/// Trait for places that have delay constraint information.
pub trait HasDelay {
    /// Returns a reference to the delay constraints for this place.
    fn delay(&self) -> &DelayPair;
}

impl AsRef<Place> for Place {
    fn as_ref(&self) -> &Place {
        self
    }
}

impl AsMut<Place> for Place {
    fn as_mut(&mut self) -> &mut Place {
        self
    }
}

/// Generic HBCN graph type parameterized by node and edge types.
///
/// The HBCN is implemented as a stable graph from the `petgraph` crate, which provides
/// efficient graph operations while maintaining stable node indices even after graph modifications.
pub type HBCN<T, P> = StableGraph<T, P>;

/// Structural HBCN representation before timing analysis.
///
/// This is the initial HBCN structure created from a structural graph. Nodes are [`Transition`]s
/// and edges are [`WeightedPlace`]s. This representation captures the circuit structure and topology
/// but does not yet contain computed timing information.
pub type StructuralHBCN = HBCN<Transition, WeightedPlace>;

/// Solved HBCN representation after timing analysis.
///
/// This is the HBCN after timing constraints have been computed. Nodes are [`TransitionEvent`]s
/// (transitions with timing information) and edges are [`DelayedPlace`]s (places with computed
/// delays and slack values). This representation is produced by constraint generation algorithms
/// and used for generating timing constraint outputs.
pub type SolvedHBCN = HBCN<TransitionEvent, DelayedPlace>;

/// A transition event with associated timing information.
///
/// This represents a transition that has been assigned a time value during timing analysis.
/// The `time` field indicates when this transition occurs in the circuit's timing schedule,
/// while `transition` identifies what type of transition (data or spacer) occurs at which
/// circuit node.
///
/// `TransitionEvent` is used in [`SolvedHBCN`] to represent transitions with resolved timing.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct TransitionEvent {
    /// The time at which this transition occurs.
    pub time: f64,
    /// The underlying transition (data or spacer).
    pub transition: Transition,
}

impl AsRef<CircuitNode> for TransitionEvent {
    fn as_ref(&self) -> &CircuitNode {
        self.transition.as_ref()
    }
}

impl AsRef<Transition> for TransitionEvent {
    fn as_ref(&self) -> &Transition {
        &self.transition
    }
}

impl From<TransitionEvent> for Transition {
    fn from(ev: TransitionEvent) -> Transition {
        ev.transition
    }
}

impl TimedEvent for TransitionEvent {
    fn time(&self) -> f64 {
        self.time
    }
}

/// A place with computed delay constraints and slack information.
///
/// This represents a place after timing analysis has determined the delay constraints
/// required to meet the cycle time requirements. The `delay` field contains the computed
/// min/max delay values, while `slack` optionally contains the timing slack for this place.
///
/// `DelayedPlace` is used in [`SolvedHBCN`] to represent places with resolved timing constraints.
///
/// # Weight Calculation
///
/// When computing weights, `DelayedPlace` uses the maximum delay from `delay.max`.
/// This ensures that computed delay constraints are used for weight calculations.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::{DelayedPlace, Place, DelayPair};
///
/// let place = Place {
///     token: true,
///     is_internal: false,
/// };
///
/// let delayed_place = DelayedPlace {
///     place,
///     delay: DelayPair {
///         min: Some(1.0),
///         max: 8.5,
///     },
///     slack: Some(1.5),
/// };
/// ```
/// A place with an associated weight/delay value.
///
/// This represents a place that has a weight (delay/cost) associated with it.
/// The `weight` field contains the delay or cost value for this place.
///
/// `WeightedPlace` is used in [`StructuralHBCN`] to represent places with weights.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::{WeightedPlace, Place};
///
/// let place = Place {
///     token: true,
///     is_internal: false,
/// };
///
/// let weighted_place = WeightedPlace {
///     place,
///     weight: 10.0,
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct WeightedPlace {
    /// The underlying place structure.
    pub place: Place,
    /// The delay/cost weight of this place.
    pub weight: f64,
}

impl HasWeight for WeightedPlace {
    fn weight(&self) -> f64 {
        self.weight
    }
}

impl AsRef<Place> for WeightedPlace {
    fn as_ref(&self) -> &Place {
        &self.place
    }
}

impl AsMut<Place> for WeightedPlace {
    fn as_mut(&mut self) -> &mut Place {
        &mut self.place
    }
}

impl From<WeightedPlace> for Place {
    fn from(w: WeightedPlace) -> Place {
        w.place
    }
}

#[derive(Debug, Clone, Default)]
pub struct DelayedPlace {
    /// The underlying place structure.
    pub place: Place,
    /// Computed delay constraints (min and/or max delay values).
    pub delay: DelayPair,
    /// Optional timing slack for this place.
    pub slack: Option<f64>,
}

impl HasWeight for DelayedPlace {
    fn weight(&self) -> f64 {
        self.delay.max
    }
}

impl HasDelay for DelayedPlace {
    fn delay(&self) -> &DelayPair {
        &self.delay
    }
}

impl AsRef<Place> for DelayedPlace {
    fn as_ref(&self) -> &Place {
        &self.place
    }
}

impl AsMut<Place> for DelayedPlace {
    fn as_mut(&mut self) -> &mut Place {
        &mut self.place
    }
}

impl From<DelayedPlace> for Place {
    fn from(val: DelayedPlace) -> Self {
        val.place
    }
}

impl SlackablePlace for DelayedPlace {
    fn slack(&self) -> f64 {
        self.slack.unwrap_or(0.0)
    }
}

/// Validate an HBCN according to the pairing and marking rules.
///
/// The validation rules are:
/// 1. No edge should connect a node to itself (no self-loops)
/// 2. Every place connecting Data(a) to Data(b) must be paired with a Data(b) to Spacer(a) place
/// 3. Every Spacer(a) to Spacer(b) must be paired with a Spacer(b) to Data(a) place
/// 4. For every Data(a) to Data(b) there must exist a Spacer(a) to Spacer(b) and vice-versa
/// 5. Exactly one of the 4 aforementioned places must be marked
///
/// # Arguments
///
/// * `hbcn` - The HBCN graph to validate
///
/// # Returns
///
/// Returns `Ok(())` if validation passes, or an `Error` if validation fails.
///
/// # Example
///
/// ```no_run
/// use hbcn::hbcn::{StructuralHBCN, validate_hbcn};
///
/// let hbcn = StructuralHBCN::default();
/// if let Err(e) = validate_hbcn(&hbcn) {
///     eprintln!("Validation failed: {}", e);
/// }
/// ```
pub fn validate_hbcn<T: AsRef<Transition>, P: MarkablePlace>(hbcn: &HBCN<T, P>) -> Result<()> {
    // Check for self-loops (edges connecting a node to itself) and build edge_map
    let edge_map: HashMap<(NodeIndex, NodeIndex), EdgeIndex> = hbcn
        .edge_indices()
        .filter_map(|edge_idx| {
            hbcn.edge_endpoints(edge_idx).map(|(src, dst)| {
                if src == dst {
                    let transition = hbcn[src].as_ref();
                    Err(format!(
                        "Invalid HBCN: found self-loop - transition {} has an edge connecting to itself",
                        transition
                    ))
                } else {
                    Ok(((src, dst), edge_idx))
                }
            })
        })
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Get a map of circuit nodes to their Data and Spacer transition nodes
    let (node_to_data, node_to_spacer) = hbcn.node_indices().fold(
        (
            HashMap::<&CircuitNode, NodeIndex>::new(),
            HashMap::<&CircuitNode, NodeIndex>::new(),
        ),
        |(mut data_map, mut spacer_map), node_idx| {
            let transition = &hbcn[node_idx];
            match transition.as_ref() {
                Transition::Data(circuit_node) => {
                    data_map.insert(circuit_node, node_idx);
                }
                Transition::Spacer(circuit_node) => {
                    spacer_map.insert(circuit_node, node_idx);
                }
            }
            (data_map, spacer_map)
        },
    );

    // Collect all channel pairs by examining all edges
    // Each edge type reveals a channel pair:
    // - Data(a) -> Data(b) reveals channel (a, b)
    // - Spacer(a) -> Spacer(b) reveals channel (a, b)
    // - Data(b) -> Spacer(a) reveals channel (a, b) (backward pairing)
    // - Spacer(b) -> Data(a) reveals channel (a, b) (backward pairing)
    let channel_pairs: HashSet<(CircuitNode, CircuitNode)> = hbcn
        .edge_indices()
        .filter_map(|edge_idx| hbcn.edge_endpoints(edge_idx))
        .map(|(src_idx, dst_idx)| {
            let src_transition = hbcn[src_idx].as_ref();
            let dst_transition = hbcn[dst_idx].as_ref();

            match (src_transition, dst_transition) {
                // Data(a) -> Data(b) reveals channel (a, b)
                (Transition::Data(node_a), Transition::Data(node_b)) => {
                    (node_a.clone(), node_b.clone())
                }
                // Spacer(a) -> Spacer(b) reveals channel (a, b)
                (Transition::Spacer(node_a), Transition::Spacer(node_b)) => {
                    (node_a.clone(), node_b.clone())
                }
                // Data(b) -> Spacer(a) reveals channel (a, b) (backward pairing)
                (Transition::Data(node_b), Transition::Spacer(node_a)) => {
                    (node_a.clone(), node_b.clone())
                }
                // Spacer(b) -> Data(a) reveals channel (a, b) (backward pairing)
                (Transition::Spacer(node_b), Transition::Data(node_a)) => {
                    (node_a.clone(), node_b.clone())
                }
            }
        })
        .collect();

    // Validate each channel pair once
    // For a channel between node_a and node_b, we need exactly 4 places:
    // 1. Data(node_a) -> Data(node_b): forward data flow
    // 2. Data(node_b) -> Spacer(node_a): backward acknowledgment for data
    // 3. Spacer(node_a) -> Spacer(node_b): forward spacer flow
    // 4. Spacer(node_b) -> Data(node_a): backward acknowledgment for spacer
    // Exactly one of these 4 places must be marked (have a token)
    channel_pairs.into_iter().try_fold((), |_, (node_a, node_b)| {
        // Get transition node indices for both circuit nodes
        let data_a = node_to_data.get(&node_a)
            .ok_or_else(|| anyhow::anyhow!("Channel validation failed: missing Data transition for node {}", node_a))?;
        let data_b = node_to_data.get(&node_b)
            .ok_or_else(|| anyhow::anyhow!("Channel validation failed: missing Data transition for node {}", node_b))?;
        let spacer_a = node_to_spacer.get(&node_a)
            .ok_or_else(|| anyhow::anyhow!("Channel validation failed: missing Spacer transition for node {}", node_a))?;
        let spacer_b = node_to_spacer.get(&node_b)
            .ok_or_else(|| anyhow::anyhow!("Channel validation failed: missing Spacer transition for node {}", node_b))?;

        // Find all four required places and check their markings
        let forward_data_place = edge_map.get(&(*data_a, *data_b));
        let backward_data_ack_place = edge_map.get(&(*data_b, *spacer_a));
        let forward_spacer_place = edge_map.get(&(*spacer_a, *spacer_b));
        let backward_spacer_ack_place = edge_map.get(&(*spacer_b, *data_a));

        // Check which places are marked (have tokens)
        let forward_data_marked = forward_data_place
            .map(|idx| hbcn[*idx].is_marked())
            .unwrap_or(false);
        let backward_data_ack_marked = backward_data_ack_place
            .map(|idx| hbcn[*idx].is_marked())
            .unwrap_or(false);
        let forward_spacer_marked = forward_spacer_place
            .map(|idx| hbcn[*idx].is_marked())
            .unwrap_or(false);
        let backward_spacer_ack_marked = backward_spacer_ack_place
            .map(|idx| hbcn[*idx].is_marked())
            .unwrap_or(false);

        // Validate that all required places exist
        if forward_data_place.is_none() {
            bail!(
                "Channel from {} to {} is missing the forward data place: Data({}) -> Data({})",
                node_a,
                node_b,
                node_a,
                node_b
            );
        }

        if backward_data_ack_place.is_none() {
            bail!(
                "Channel from {} to {} is missing the backward acknowledgment place: Data({}) -> Spacer({})",
                node_a,
                node_b,
                node_b,
                node_a
            );
        }

        if forward_spacer_place.is_none() {
            bail!(
                "Channel from {} to {} is missing the forward spacer place: Spacer({}) -> Spacer({})",
                node_a,
                node_b,
                node_a,
                node_b
            );
        }

        if backward_spacer_ack_place.is_none() {
            bail!(
                "Channel from {} to {} is missing the backward spacer acknowledgment place: Spacer({}) -> Data({})",
                node_a,
                node_b,
                node_b,
                node_a
            );
        }

        // Count how many places are marked
        let marked_count = [
            forward_data_marked,
            backward_data_ack_marked,
            forward_spacer_marked,
            backward_spacer_ack_marked,
        ]
        .iter()
        .filter(|&&marked| marked)
        .count();

        if marked_count == 0 {
            bail!(
                "Channel from {} to {} has no marked places. Exactly one of the four places must be marked (have a token).",
                node_a,
                node_b
            );
        }

        if marked_count > 1 {
            bail!(
                "Channel from {} to {} has {} marked places. Exactly one of the four places must be marked (have a token).",
                node_a,
                node_b,
                marked_count
            );
        }

        Ok(())
    })
}
