//! Expansion tools for converting structural graphs to HBCN representation.
//!
//! This module provides functionality for converting structural graphs to HBCN
//! representation and serialising the result to the HBCN format.
//!
//! # Main Operations
//!
//! - **[`expand_main`]**: Converts a structural graph to HBCN representation
//!   and writes it to an output file.
//!
//! # Workflow
//!
//! 1. Parse input as a structural graph
//! 2. Convert to HBCN representation using `from_structural_graph`
//! 3. Serialize the HBCN to the output format
//!
//! # Example
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use hbcn::expand::{ExpandArgs, expand_main};
//!
//! let args = ExpandArgs {
//!     input: "circuit.graph".into(),
//!     output: "circuit.hbcn".into(),
//!     forward_completion: false,
//! };
//!
//! expand_main(args)?;
//! # Ok(())
//! # }
//! ```

use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::*;
use clap::Parser;
use petgraph::graph::NodeIndex;

use crate::{
    hbcn::{serialisation, *},
    read_file,
};

/// Command-line arguments for the expand command.
#[derive(Parser, Debug)]
pub struct ExpandArgs {
    /// Structural graph input file
    pub input: PathBuf,

    /// HBCN output file
    #[clap(short, long)]
    pub output: PathBuf,

    /// Enable forward completion delay calculation
    #[clap(long)]
    pub forward_completion: bool,
}

/// Convert a structural graph to HBCN representation and write to output file.
///
/// This is the main entry point for expansion. It:
///
/// 1. Reads and parses the structural graph
/// 2. Converts to HBCN representation using `from_structural_graph`
/// 3. Serialises the HBCN to the output file in the parser format
///
/// # Arguments
///
/// * `args` - Configuration including input file, output path, and forward completion flag
///
/// # Example
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use hbcn::expand::{ExpandArgs, expand_main};
///
/// let args = ExpandArgs {
///     input: "circuit.graph".into(),
///     output: "circuit.hbcn".into(),
///     forward_completion: false,
/// };
///
/// expand_main(args)?;
/// # Ok(())
/// # }
/// ```
pub fn expand_main(args: ExpandArgs) -> Result<()> {
    let ExpandArgs {
        input,
        output,
        forward_completion,
    } = args;

    // Read and parse the structural graph
    let graph = read_file(&input)?;

    // Convert to HBCN representation
    let hbcn = from_structural_graph(&graph, forward_completion)
        .ok_or_else(|| anyhow!("Failed to convert structural graph to HBCN"))?;

    // Convert StructuralHBCN (WeightedPlace) to SolvedHBCN (DelayedPlace) for serialisation
    let mut converted_hbcn = SolvedHBCN::new();
    let node_map: HashMap<NodeIndex, NodeIndex> = hbcn
        .node_indices()
        .map(|idx| {
            let transition = hbcn[idx].clone();
            let new_idx = converted_hbcn.add_node(TransitionEvent {
                time: 0.0,
                transition,
            });
            (idx, new_idx)
        })
        .collect();

    for edge_idx in hbcn.edge_indices() {
        let (src, dst) = hbcn
            .edge_endpoints(edge_idx)
            .expect("Edge should have valid endpoints");
        let weighted_place = &hbcn[edge_idx];

        let delayed_place: DelayedPlace = weighted_place.clone().into();

        converted_hbcn.add_edge(node_map[&src], node_map[&dst], delayed_place);
    }

    // Serialise the HBCN to the output file
    let serialised = serialisation::serialise_hbcn(&converted_hbcn);
    fs::write(&output, serialised)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hbcn::serialisation, structural_graph::parse};
    use petgraph::graph::NodeIndex;
    use std::collections::HashMap;

    #[test]
    fn test_expand_simple_graph() {
        let graph = parse(
            r#"
            Port "input" [("output", 100)]
            Port "output" []
        "#,
        )
        .unwrap();

        let hbcn = from_structural_graph(&graph, false).unwrap();

        // Convert to DelayedPlace as expand_main does
        let mut converted_hbcn = SolvedHBCN::new();
        let node_map: HashMap<NodeIndex, NodeIndex> = hbcn
            .node_indices()
            .map(|idx| {
                let transition = hbcn[idx].clone();
                let new_idx = converted_hbcn.add_node(TransitionEvent {
                    time: 0.0,
                    transition,
                });
                (idx, new_idx)
            })
            .collect();

        for edge_idx in hbcn.edge_indices() {
            let (src, dst) = hbcn
                .edge_endpoints(edge_idx)
                .expect("Edge should have valid endpoints");
            let weighted_place = &hbcn[edge_idx];

            let delayed_place: DelayedPlace = weighted_place.clone().into();

            converted_hbcn.add_edge(node_map[&src], node_map[&dst], delayed_place);
        }

        let output = serialisation::serialise_hbcn(&converted_hbcn);

        // Should contain transitions for both nodes
        assert!(output.contains("+{input}"));
        assert!(output.contains("-{input}"));
        assert!(output.contains("+{output}"));
        assert!(output.contains("-{output}"));

        // Should contain arrow notation
        assert!(output.contains(" => "));

        // Should have tokens (one place marked per channel)
        assert!(output.contains("* "));
    }
}
