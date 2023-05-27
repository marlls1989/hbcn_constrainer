mod analyse;
mod constrain;
mod hbcn;
mod sdc;
mod structural_graph;

use std::{error::Error, fmt, fs, path::Path};

use crate::analyse::*;
use crate::constrain::*;
use anyhow::*;
use clap::Parser;

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

#[derive(Debug, Parser)]
#[clap(name = "HBCN Tools", about = "Pulsar HBCN timing analysis tools")]
enum CLIArguments {
    /// Find longest path depth in the HBCN
    Depth(DepthArgs),
    /// Estimate the virtual-delay cycle-time, it can be used to tune the circuit performance.
    Analyse(AnalyseArgs),
    /// Constrain the cycle-time using continous proportional constraints.
    Constrain(ConstrainArgs),
}

fn read_file(file_name: &Path) -> Result<structural_graph::StructuralGraph> {
    let file = fs::read_to_string(file_name)?;
    Ok(structural_graph::parse(&file)?)
}

fn main() -> Result<()> {
    let args = CLIArguments::from_args();

    match args {
        CLIArguments::Constrain(args) => constrain_main(args),
        CLIArguments::Analyse(args) => analyse_main(args),
        CLIArguments::Depth(args) => depth_main(args),
    }
}
