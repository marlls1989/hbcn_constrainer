//! HBCN (Half-buffer Channel Network) timing analysis and constraint generation library
//!
//! This library provides a comprehensive toolkit for analyzing and constraining timing in
//! Half-Buffer Channel Networks, which are used to model asynchronous digital circuits.
//!
//! # Overview
//!
//! The HBCN Constrainer is part of the **Pulsar** framework for asynchronous circuit synthesis.
//! It takes structural circuit descriptions and generates timing constraints suitable for
//! EDA tools like Cadence Genus.
//!
//! # Main Workflows
//!
//! The library supports three main operations:
//!
//! 1. **Expansion** ([`expand`]): Convert structural graphs to HBCN representation
//! 2. **Analysis** ([`analyse`]): Estimate cycle times and identify critical paths (supports depth analysis with `--depth` flag)
//! 3. **Constraint Generation** ([`constrain`]): Generate SDC timing constraints for synthesis
//!
//! # Usage Example
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use hbcn::{read_file, from_structural_graph};
//! use std::path::Path;
//!
//! // Read a structural graph from a file
//! let graph = read_file(Path::new("circuit.graph"))?;
//!
//! // Convert to HBCN for analysis
//! let hbcn = from_structural_graph(&graph, false)
//!     .expect("Failed to convert to HBCN");
//!
//! // Use hbcn for timing analysis or constraint generation
//! # Ok(())
//! # }
//! ```
//!
//! # Modules
//!
//! - **[`structural_graph`]**: Parsing and representation of structural circuit graphs
//! - **[`hbcn`]**: Core HBCN data structures, types (like [`CircuitNode`] and [`DelayPair`]),
//!   and conversion from structural graphs. Most HBCN-related types are re-exported from this module.
//! - **[`expand`]**: Conversion of structural graphs to HBCN representation and serialization
//! - **[`analyse`]**: Cycle time analysis and critical path identification
//! - **[`constrain`]**: Timing constraint generation using LP optimization
//! - **[`lp_solver`]**: Linear programming solver abstraction layer
//!
//! # Re-exports
//!
//! The library re-exports commonly used types and functions:
//!
//! - All HBCN types are available via the `hbcn` module, including [`CircuitNode`], [`DelayPair`],
//!   [`Transition`], [`Place`], etc. These are re-exported through `pub use hbcn::*`.
//! - [`Symbol`] type is re-exported from [`structural_graph`] for convenient use

use anyhow::Result;
use clap::Parser;
use std::{error::Error, fmt, fs, path::Path};

pub mod analyse;
pub mod constrain;
pub mod expand;
pub mod hbcn;
pub mod lp_solver;
pub mod structural_graph;

// Re-export the main functions for easy access
pub use analyse::{AnalyseArgs, analyse_main};
pub use constrain::{ConstrainArgs, constrain_main};
pub use expand::{ExpandArgs, expand_main};
pub use hbcn::*;
pub use structural_graph::Symbol;

/// Application-level errors that can occur during HBCN processing.
#[derive(Debug, PartialEq, Eq)]
pub enum AppError {
    /// The constraint generation problem is infeasible (no solution exists).
    Infeasible,
    /// No output format was specified for constraint generation.
    NoOutput,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NoOutput => write!(f, "At least one output must be selected."),
            AppError::Infeasible => write!(f, "Problem Infeasible"),
        }
    }
}

impl Error for AppError {}

/// Reads and parses a structural graph from a file.
///
/// This is a convenience function that reads a structural graph description from a file
/// and parses it into a [`structural_graph::StructuralGraph`] representation.
///
/// # Arguments
///
/// * `file_name` - Path to the structural graph file (typically `.graph` format)
///
/// # Returns
///
/// Returns the parsed structural graph, or an error if the file cannot be read or parsed.
///
/// # Example
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use hbcn::read_file;
/// use std::path::Path;
///
/// let graph = read_file(Path::new("circuit.graph"))?;
/// # Ok(())
/// # }
/// ```
pub fn read_file(file_name: &Path) -> Result<structural_graph::StructuralGraph> {
    let file = fs::read_to_string(file_name)?;
    Ok(structural_graph::parse(&file)?)
}

/// Command-line interface arguments for the HBCN tools.
///
/// This enum defines the main commands available:
/// - `Expand`: Convert structural graphs to HBCN representation
/// - `Analyse`: Estimate cycle time and analyze critical paths
/// - `Constrain`: Generate timing constraints for synthesis
#[derive(Debug, Parser)]
#[clap(
    name = "HBCN Tools",
    about = "Pulsar Half-buffer Channel Network timing analysis tools"
)]
pub enum CLIArguments {
    /// Convert a structural graph to HBCN representation.
    Expand(ExpandArgs),
    /// Estimate the virtual-delay cycle-time, it can be used to tune the circuit performance.
    /// Use --depth to analyze cycle depth instead of weighted cycle time.
    Analyse(AnalyseArgs),
    /// Constrain the cycle-time using continous proportional constraints.
    Constrain(ConstrainArgs),
}
