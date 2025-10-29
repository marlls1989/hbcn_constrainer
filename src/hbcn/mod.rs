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
//! - **Forward places** (`backward = false`): Model forward data flow timing
//! - **Backward places** (`backward = true`): Model acknowledgment/return path timing
//! - **Token marking** (`token`): Indicates whether a place initially contains a token
//! - **Weight** (`weight`): Represents the delay/cost associated with traversing the place
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
//! - **[`HasTransition`]**: Types that have an associated transition
//! - **[`HasCircuitNode`]**: Types that reference a circuit node
//! - **[`Named`]**: Types that have a name (derived from circuit nodes)
//! - **[`TimedEvent`]**: Types that have a time value
//! - **[`MarkablePlace`]**: Places that can be marked/unmarked (token state)
//! - **[`WeightedPlace`]**: Places that have a weight/delay
//! - **[`SlackablePlace`]**: Places that have computed slack values
//! - **[`HasPlace`]**: Types that contain or are a place
//!
//! These traits enable generic algorithms that work across different HBCN representations.
//!
//! # Re-exported Types
//!
//! This module also provides simplified versions of types for use with HBCN:
//!
//! - **[`CircuitNode`]**: Simplified circuit node representation (without cost field)
//! - **[`DelayPair`]**: Min/max delay constraint representation used in timing analysis

pub mod parser;
pub mod structural_graph;
pub use structural_graph::from_structural_graph;

use crate::Symbol;
use petgraph::stable_graph::StableGraph;
use std::fmt;
use crate::structural_graph::CircuitNode as StructuralCircuitNode;

/// Trait for types that have an associated transition.
pub trait HasTransition {
    /// Returns a reference to the associated transition.
    fn transition(&self) -> &Transition;
}

/// Trait for types that have a name.
///
/// Implemented automatically for types that implement [`HasCircuitNode`],
/// since circuit nodes provide names.
pub trait Named {
    /// Returns a reference to the name of this element.
    fn name(&self) -> &Symbol;
}

/// Trait for types that reference a circuit node.
///
/// Circuit nodes represent physical elements in the structural circuit graph.
pub trait HasCircuitNode {
    /// Returns a reference to the associated circuit node.
    fn circuit_node(&self) -> &CircuitNode;
}

/// Represents a delay pair with optional minimum and maximum delay constraints.
///
/// This type is used throughout the HBCN constraint generation process to specify
/// timing requirements on paths between circuit nodes.
///
/// # Fields
///
/// - `min`: Optional minimum delay constraint (in time units)
/// - `max`: Optional maximum delay constraint (in time units)
///
/// # Example
///
/// ```
/// use hbcn::hbcn::DelayPair;
///
/// // Path with both min and max delays
/// let constraint = DelayPair {
///     min: Some(1.0),
///     max: Some(8.5),
/// };
///
/// // Path with only max delay
/// let max_only = DelayPair {
///     min: None,
///     max: Some(10.0),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct DelayPair {
    /// Optional minimum delay constraint.
    pub min: Option<f64>,
    /// Optional maximum delay constraint.
    pub max: Option<f64>,
}

impl DelayPair {
    /// Create a new delay pair with the specified min and max values.
    pub fn new(min: Option<f64>, max: Option<f64>) -> Self {
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
/// let register = CircuitNode::Register {
///     name: DefaultAtom::from("reg1"),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CircuitNode {
    /// External interface port.
    Port(Symbol),
    /// Register component.
    Register {
        /// Register name/identifier.
        name: Symbol,
    },
}

impl From<StructuralCircuitNode> for CircuitNode {
    fn from(node: StructuralCircuitNode) -> Self {
        match node {
            StructuralCircuitNode::Port(name) => CircuitNode::Port(name),
            StructuralCircuitNode::Register { name, .. } => CircuitNode::Register { name },
        }
    }
}

impl Named for CircuitNode {
    fn name(&self) -> &Symbol {
        match self {
            CircuitNode::Port(name) => name,
            CircuitNode::Register { name } => name,
        }
    }
}

impl fmt::Display for CircuitNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitNode::Port(name) => write!(f, "Port \"{}\"", name),
            CircuitNode::Register { name } => write!(f, "Register \"{}\"", name),
        }
    }
}

/// Trait for types that have an associated time value.
pub trait TimedEvent {
    /// Returns the time value associated with this event.
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

/// Represents a place (edge) in the HBCN graph.
///
/// Places model timing dependencies between transitions in the HBCN. They represent
/// the channels and synchronization points in the asynchronous circuit, capturing the
/// timing relationships needed for correct handshaking behavior.
///
/// # Fields
///
/// - **`backward`**: Whether this place is in the backward (acknowledgment) direction.
///   - `false`: Forward place (data flow direction)
///   - `true`: Backward place (acknowledgment/return direction)
///
/// - **`token`**: Whether this place initially contains a token. Tokens represent the
///   initial state of the place in the handshaking protocol and affect the initial marking
///   of the HBCN graph.
///
/// - **`weight`**: The delay or cost associated with traversing this place. In timing
///   analysis, this represents the minimum time required for a transition to propagate
///   through this place.
///
/// - **`is_internal`**: Whether this place represents an internal connection (between
///   internal circuit components) versus an external channel.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::Place;
///
/// let forward_place = Place {
///     backward: false,
///     token: true,
///     weight: 10.0,
///     is_internal: false,
/// };
/// ```
#[derive(PartialEq, Debug, Clone, Default)]
pub struct Place {
    /// Whether this place is in the backward direction.
    pub backward: bool,
    /// Whether this place initially contains a token.
    pub token: bool,
    /// The delay/cost weight of this place.
    pub weight: f64,
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

/// Trait for places that have a computed slack value.
///
/// Slack represents the difference between required arrival time and actual arrival time,
/// indicating how much timing margin exists for this place.
pub trait SlackablePlace {
    /// Returns the slack value for this place.
    fn slack(&self) -> f64;
}

/// Trait for places that have a weight/delay value.
pub trait WeightedPlace {
    /// Returns the weight (delay/cost) of this place.
    fn weight(&self) -> f64;
}

/// Trait for types that contain or are a place.
pub trait HasPlace {
    /// Returns a reference to the place.
    fn place(&self) -> &Place;
    /// Returns a mutable reference to the place.
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

/// Generic HBCN graph type parameterized by node and edge types.
///
/// The HBCN is implemented as a stable graph from the `petgraph` crate, which provides
/// efficient graph operations while maintaining stable node indices even after graph modifications.
pub type HBCN<T, P> = StableGraph<T, P>;

/// Structural HBCN representation before timing analysis.
///
/// This is the initial HBCN structure created from a structural graph. Nodes are [`Transition`]s
/// and edges are [`Place`]s. This representation captures the circuit structure and topology
/// but does not yet contain computed timing information.
pub type StructuralHBCN = HBCN<Transition, Place>;

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
/// When computing weights, `DelayedPlace` uses the maximum delay from `delay.max` if available,
/// otherwise it falls back to the base `place.weight`. This ensures that computed delay constraints
/// take precedence over initial weight estimates.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::{DelayedPlace, Place, DelayPair};
///
/// let place = Place {
///     backward: false,
///     token: true,
///     weight: 10.0,
///     is_internal: false,
/// };
///
/// let delayed_place = DelayedPlace {
///     place,
///     delay: DelayPair {
///         min: Some(1.0),
///         max: Some(8.5),
///     },
///     slack: Some(1.5),
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct DelayedPlace {
    /// The underlying place structure.
    pub place: Place,
    /// Computed delay constraints (min and/or max delay values).
    pub delay: DelayPair,
    /// Optional timing slack for this place.
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
