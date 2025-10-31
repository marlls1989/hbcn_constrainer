//! Serialization functions for HBCN graphs.
//!
//! This module provides functions to serialize HBCN graphs to the format
//! defined by the parser grammar.

use crate::hbcn::{DelayPair, HasDelay, MarkablePlace, Named, Transition};
use petgraph::stable_graph::StableGraph;
use std::fmt::{self};

/// Serialize an HBCN to the format defined by the parser grammar into the provided writer.
///
/// See `serialize_hbcn` for format details.
pub fn serialize_hbcn_to<N, P, W>(
    hbcn: &StableGraph<N, P>,
    writer: &mut W,
) -> fmt::Result
where
    N: AsRef<Transition>,
    P: MarkablePlace + HasDelay,
    W: fmt::Write,
{
    serialize_hbcn_internal(hbcn, |node| node.as_ref(), writer)
}

/// Serialize an HBCN to the format defined by the parser grammar.
///
/// This function serializes the edges of an HBCN graph where:
/// - Nodes are either [`Transition`]s directly or implement `AsRef<Transition>`
/// - Edges implement [`MarkablePlace`] and [`HasDelay`]
///
/// The output format matches the parser grammar:
/// - Edges: `source => target : delay` or `* source => target : delay` (if token is marked)
/// - Transitions: `+{"name"}` for Data, `-{"name"}` for Spacer
/// - DelayPair: `(min, max)` when min is present, or `max` when min is absent
///
/// # Arguments
///
/// * `hbcn` - The HBCN graph to serialize
///
/// # Returns
///
/// A string representation of the graph in the parser format.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::serialisation::serialize_hbcn;
/// use hbcn::hbcn::{TransitionEvent, DelayedPlace, Place, DelayPair, CircuitNode, Transition};
/// use petgraph::stable_graph::StableGraph;
/// use string_cache::DefaultAtom;
///
/// let mut graph: StableGraph<TransitionEvent, DelayedPlace> = StableGraph::new();
/// let n1 = graph.add_node(TransitionEvent {
///     time: 0.0,
///     transition: Transition::Data(CircuitNode::Port(DefaultAtom::from("node1"))),
/// });
/// let n2 = graph.add_node(TransitionEvent {
///     time: 1.0,
///     transition: Transition::Spacer(CircuitNode::Port(DefaultAtom::from("node2"))),
/// });
/// graph.add_edge(n1, n2, DelayedPlace {
///     place: Place { backward: false, token: true, is_internal: false },
///     delay: DelayPair { min: Some(1.0), max: 2.0 },
///     slack: None,
/// });
///
/// let output = serialize_hbcn(&graph);
/// ```
pub fn serialize_hbcn<N, P>(
    hbcn: &StableGraph<N, P>,
) -> String
where
    N: AsRef<Transition>,
    P: MarkablePlace + HasDelay,
{
    let mut out = String::new();
    // Infallible for String
    let _ = serialize_hbcn_to(hbcn, &mut out);
    out
}

/// Internal helper that works with a transition extractor function and a writer.
fn serialize_hbcn_internal<N, P, W>(
    hbcn: &StableGraph<N, P>,
    get_transition: impl Fn(&N) -> &Transition,
    writer: &mut W,
) -> std::fmt::Result
where
    P: MarkablePlace + HasDelay,
    W: fmt::Write,
{
    let mut first = true;

    for edge_idx in hbcn.edge_indices() {
        let (source_idx, target_idx) = hbcn
            .edge_endpoints(edge_idx)
            .expect("Edge should have valid endpoints");
        let edge = &hbcn[edge_idx];

        if !first {
            writer.write_char('\n')?;
        }
        first = false;

        // Token prefix
        if edge.is_marked() {
            writer.write_str("* ")?;
        } else {
            writer.write_str("  ")?;
        }

        // Source transition
        write_transition(get_transition(&hbcn[source_idx]), writer)?;

        // Arrow
        writer.write_str(" => ")?;

        // Target transition
        write_transition(get_transition(&hbcn[target_idx]), writer)?;

        // Delay
        writer.write_str(" : ")?;
        write_delay_pair(edge.delay(), writer)?;
    }

    Ok(())
}

/// Serialize an HBCN where nodes are directly Transitions.
///
/// This is a convenience function for HBCNs with `Transition` nodes.
pub fn serialize_hbcn_transition<P>(
    hbcn: &StableGraph<Transition, P>,
) -> String
where
    P: MarkablePlace + HasDelay,
{
    let mut out = String::new();
    let _ = serialize_hbcn_internal(hbcn, |t| t, &mut out);
    out
}

fn write_transition<T: AsRef<Transition>, W: fmt::Write>(transition: T, writer: &mut W) -> fmt::Result {
    let t = transition.as_ref();
    match t {
        Transition::Data(circuit_node) => {
            write!(writer, "+{{\"{}\"}}", circuit_node.name().as_ref())
        }
        Transition::Spacer(circuit_node) => {
            write!(writer, "-{{\"{}\"}}", circuit_node.name().as_ref())
        }
    }
}

fn write_delay_pair<W: fmt::Write>(delay: &DelayPair, writer: &mut W) -> fmt::Result {
    match delay.min {
        Some(min) => write!(writer, "({},{})", min, delay.max),
        None => write!(writer, "{}", delay.max),
    }
}

