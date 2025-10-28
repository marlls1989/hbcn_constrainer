//! HBCN (Half-buffer Channel Network) timing analysis and constraint generation library
//!
//! This library provides tools for analyzing and constraining timing in HBCN circuits.

use anyhow::Result;
use clap::Parser;
use std::{error::Error, fmt, fs, path::Path};

pub mod analyse;
pub mod constrain;
pub mod hbcn;
pub mod lp_solver;
pub mod structural_graph;

// Re-export the main functions for easy access
pub use analyse::{AnalyseArgs, DepthArgs, analyse_main, depth_main};
pub use constrain::{ConstrainArgs, constrain_main};
pub use hbcn::*;
pub use structural_graph::*;

#[derive(Debug, PartialEq, Eq)]
pub enum AppError {
    Infeasible,
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

pub fn read_file(file_name: &Path) -> Result<structural_graph::StructuralGraph> {
    let file = fs::read_to_string(file_name)?;
    Ok(structural_graph::parse(&file)?)
}

#[derive(Debug, Parser)]
#[clap(
    name = "HBCN Tools",
    about = "Pulsar Half-buffer Channel Network timing analysis tools"
)]
pub enum CLIArguments {
    /// Find longest path depth in the HBCN
    Depth(DepthArgs),
    /// Estimate the virtual-delay cycle-time, it can be used to tune the circuit performance.
    Analyse(AnalyseArgs),
    /// Constrain the cycle-time using continous proportional constraints.
    Constrain(ConstrainArgs),
}
