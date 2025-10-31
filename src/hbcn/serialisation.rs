//! Serialisation functions for HBCN graphs.
//!
//! This module provides functions to serialise HBCN graphs to the format
//! defined by the parser grammar.

use crate::hbcn::{DelayPair, HasDelay, MarkablePlace, Named, Transition};
use petgraph::stable_graph::StableGraph;
use std::fmt::{self};

/// Serialise an HBCN to the format defined by the parser grammar into the provided writer.
///
/// See `serialise_hbcn` for format details.
pub fn serialise_hbcn_to<N, P, W>(hbcn: &StableGraph<N, P>, writer: &mut W) -> fmt::Result
where
    N: AsRef<Transition>,
    P: MarkablePlace + HasDelay,
    W: fmt::Write,
{
    serialise_hbcn_internal(hbcn, |node| node.as_ref(), writer)
}

/// Serialise an HBCN to the format defined by the parser grammar.
///
/// This function serialises the edges of an HBCN graph where:
/// - Nodes are either [`Transition`]s directly or implement `AsRef<Transition>`
/// - Edges implement [`MarkablePlace`] and [`HasDelay`]
///
/// The output format matches the parser grammar:
/// - Edges: `source => target : delay` or `* source => target : delay` (if token is marked)
/// - Transitions: `+{name}` for Data, `-{name}` for Spacer (TCL-escaped strings)
/// - DelayPair: `(min, max)` when min is present, or `max` when min is absent
///
/// # Arguments
///
/// * `hbcn` - The HBCN graph to serialise
///
/// # Returns
///
/// A string representation of the graph in the parser format.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::serialisation::serialise_hbcn;
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
///     place: Place { token: true, is_internal: false },
///     delay: DelayPair { min: Some(1.0), max: 2.0 },
///     slack: None,
/// });
///
/// let output = serialise_hbcn(&graph);
/// ```
pub fn serialise_hbcn<N, P>(hbcn: &StableGraph<N, P>) -> String
where
    N: AsRef<Transition>,
    P: MarkablePlace + HasDelay,
{
    let mut out = String::new();
    // Infallible for String
    let _ = serialise_hbcn_to(hbcn, &mut out);
    out
}

/// Internal helper that works with a transition extractor function and a writer.
fn serialise_hbcn_internal<N, P, W>(
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

/// Serialise an HBCN where nodes are directly Transitions.
///
/// This is a convenience function for HBCNs with `Transition` nodes.
pub fn serialise_hbcn_transition<P>(hbcn: &StableGraph<Transition, P>) -> String
where
    P: MarkablePlace + HasDelay,
{
    let mut out = String::new();
    let _ = serialise_hbcn_internal(hbcn, |t| t, &mut out);
    out
}

fn write_transition<T: AsRef<Transition>, W: fmt::Write>(
    transition: T,
    writer: &mut W,
) -> fmt::Result {
    let t = transition.as_ref();
    match t {
        Transition::Data(circuit_node) => {
            let name = circuit_node.name().as_ref();
            // Escape braces in TCL-style: { -> \{, } -> \}
            let escaped = name.replace('{', "\\{").replace('}', "\\}");
            write!(writer, "+{{{}}}", escaped)
        }
        Transition::Spacer(circuit_node) => {
            let name = circuit_node.name().as_ref();
            // Escape braces in TCL-style: { -> \{, } -> \}
            let escaped = name.replace('{', "\\{").replace('}', "\\}");
            write!(writer, "-{{{}}}", escaped)
        }
    }
}

fn write_delay_pair<W: fmt::Write>(delay: &DelayPair, writer: &mut W) -> fmt::Result {
    match delay.min {
        Some(min) => write!(writer, "({},{})", min, delay.max),
        None => write!(writer, "{}", delay.max),
    }
}
