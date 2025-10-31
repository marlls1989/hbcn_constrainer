//! Timing constraint generation for HBCN circuits.
//!
//! This module provides functionality to generate timing constraints for EDA tools
//! (primarily Cadence Genus) from HBCN circuit analysis. It supports multiple
//! constraint generation algorithms and output formats.
//!
//! # Constraint Generation Algorithms
//!
//! - **Proportional Constraints** (default): Distributes cycle time proportionally
//!   across paths based on their virtual delays.
//!
//! - **Pseudoclock Constraints**: Uses a pseudo-clock period approach where all
//!   external paths are constrained relative to a clock period.
//!
//! # Output Formats
//!
//! The module can generate constraints in multiple formats:
//!
//! - **SDC** (Synopsys Design Constraints): Required format for Cadence Genus
//! - **CSV**: Tabular format for analysis and debugging
//! - **VCD**: Waveform format showing arrival times
//! - **Report**: Human-readable text reports with cycle analysis
//!
//! # Usage Example
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use hbcn::constrain::{ConstrainArgs, constrain_main};
//!
//! let args = ConstrainArgs {
//!     input: "circuit.hbcn".into(),
//!     structural: false,  // Read as HBCN (default)
//!     sdc: "constraints.sdc".into(),
//!     cycle_time: 10.0,
//!     minimal_delay: 1.0,
//!     csv: Some("constraints.csv".into()),
//!     rpt: Some("report.rpt".into()),
//!     vcd: None,
//!     no_proportinal: false,
//!     no_forward_completion: false,
//!     forward_margin: None,
//!     backward_margin: None,
//! };
//!
//! constrain_main(args)?;
//! # Ok(())
//! # }
//! ```

use std::{
    collections::HashMap,
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use anyhow::*;
use clap::Parser;
use ordered_float::OrderedFloat;
use prettytable::*;
use rayon::prelude::*;

use crate::{hbcn::*, read_file};

pub mod hbcn;
/// SDC (Synopsys Design Constraints) file generation for Cadence Genus.
///
/// This module generates SDC constraint files that are compatible with Cadence Genus
/// for asynchronous circuit synthesis. The SDC format specifies timing constraints
/// between circuit components using `set_min_delay` and `set_max_delay` commands.
///
/// # SDC Format
///
/// The generated SDC files include:
///
/// - Clock period definition (`create_clock`)
/// - Minimum delay constraints (`set_min_delay -through ...`)
/// - Maximum delay constraints (`set_max_delay -through ...`)
///
/// Constraints are formatted to match Cadence Genus's expected syntax for asynchronous
/// circuit components, including proper handling of ports, registers, and vector signals.
pub mod sdc;
#[cfg(test)]
mod tests;

/// Command-line arguments for the constraint generation command.
#[derive(Parser, Debug)]
pub struct ConstrainArgs {
    /// HBCN input file (default) or structural graph input file if --structural is passed
    pub input: PathBuf,

    /// Read input as a structural graph instead of an HBCN
    #[clap(long)]
    pub structural: bool,

    /// Output SDC constraints file
    #[clap(long)]
    pub sdc: PathBuf,

    /// Cycle-time constraint
    #[clap(short('t'), long)]
    pub cycle_time: f64,

    /// Minimal propagation-path delay
    #[clap(short, long)]
    pub minimal_delay: f64,

    /// Output CSV file
    #[clap(long)]
    pub csv: Option<PathBuf>,

    /// Output report file
    #[clap(long)]
    pub rpt: Option<PathBuf>,

    /// Output VCD file with arrival times
    #[clap(long)]
    pub vcd: Option<PathBuf>,

    /// Use pseudo-clock to constrain paths
    #[clap(long)]
    pub no_proportinal: bool,

    /// Don't use forward completion delay if greater than path virtual delay
    #[clap(long)]
    pub no_forward_completion: bool,

    /// Percentual margin between maximum and minimum delay in the forward path
    #[clap(long, short('f'), value_parser = clap::value_parser!(u8).range(0 .. 100))]
    pub forward_margin: Option<u8>,

    /// Minimal percentual margin between maximum and minimum delay in the backward path
    #[clap(long, short('b'), value_parser = clap::value_parser!(u8).range(0 .. 100))]
    pub backward_margin: Option<u8>,
}

/// Generate timing constraints for an HBCN circuit.
///
/// This is the main entry point for constraint generation. It:
///
/// 1. Reads and parses the input (HBCN by default, or structural graph if --structural is passed)
/// 2. If structural graph, converts it to an HBCN representation
/// 3. Generates timing constraints using the specified algorithm
/// 4. Writes constraints in the requested output formats (SDC, CSV, VCD, Report)
///
/// # Arguments
///
/// * `args` - Configuration including input file, cycle time, output paths, and algorithm options
///
/// # Outputs
///
/// - **SDC** (required): Timing constraints for Cadence Genus
/// - **CSV** (optional): Tabular constraint data
/// - **VCD** (optional): Waveform with arrival times
/// - **Report** (optional): Human-readable cycle analysis
///
/// # Example
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use hbcn::constrain::{ConstrainArgs, constrain_main};
///
/// let args = ConstrainArgs {
///     input: "circuit.hbcn".into(),
///     structural: false,  // Read as HBCN (default)
///     sdc: "output.sdc".into(),
///     cycle_time: 10.0,
///     minimal_delay: 1.0,
///     csv: None,
///     rpt: None,
///     vcd: None,
///     no_proportinal: false,
///     no_forward_completion: false,
///     forward_margin: None,
///     backward_margin: None,
/// };
///
/// constrain_main(args)?;
/// # Ok(())
/// # }
/// ```
pub fn constrain_main(args: ConstrainArgs) -> Result<()> {
    let ConstrainArgs {
        input,
        structural,
        cycle_time,
        minimal_delay,
        ref sdc,
        ref csv,
        ref rpt,
        ref vcd,
        no_proportinal,
        no_forward_completion,
        forward_margin,
        backward_margin,
    } = args;
    let forward_completion = !no_forward_completion;
    let forward_margin = forward_margin.map(|x| 1.0 - (x as f64 / 100.0));
    let backward_margin = backward_margin.map(|x| 1.0 - (x as f64 / 100.0));

    let constraints = {
        if structural {
            // Parse as structural graph
            let g = read_file(&input)?;
            let hbcn = from_structural_graph(&g, forward_completion)
                .ok_or_else(|| anyhow!("Failed to convert structural graph to StructuralHBCN"))?;

            if no_proportinal {
                hbcn::constrain_cycle_time_pseudoclock(&hbcn, cycle_time, minimal_delay)?
            } else {
                hbcn::constrain_cycle_time_proportional(
                    &hbcn,
                    cycle_time,
                    minimal_delay,
                    backward_margin,
                    forward_margin,
                )?
            }
        } else {
            // Parse as HBCN
            let file_contents = fs::read_to_string(&input)?;
            let hbcn = crate::hbcn::parser::parse_hbcn(&file_contents)?;

            if no_proportinal {
                hbcn::constrain_cycle_time_pseudoclock(&hbcn, cycle_time, minimal_delay)?
            } else {
                hbcn::constrain_cycle_time_proportional(
                    &hbcn,
                    cycle_time,
                    minimal_delay,
                    backward_margin,
                    forward_margin,
                )?
            }
        }
    };

    let hbcn = &constraints.hbcn;

    // Note: Self-reflexive path constraints have been removed

    if let Some(output) = csv {
        let mut csv_file = BufWriter::new(fs::File::create(output)?);
        let cost_map: HashMap<(CircuitNode, CircuitNode), f64> = hbcn
            .edge_indices()
            .filter_map(|ie| {
                let (is, id) = hbcn.edge_endpoints(ie)?;

                Some((
                    (
                        AsRef::<CircuitNode>::as_ref(&hbcn[is]).clone(),
                        AsRef::<CircuitNode>::as_ref(&hbcn[id]).clone(),
                    ),
                    hbcn[ie].weight(),
                ))
            })
            .collect();
        writeln!(csv_file, "src,dst,cost,max_delay,min_delay")?;
        for (key, constrain) in constraints.path_constraints.iter() {
            if let Some(cost) = cost_map.get(key) {
                let (src, dst) = key;
                write!(csv_file, "{},{},{:.0},", src.name(), dst.name(), cost,)?;
                write!(csv_file, "{:.3},", constrain.max)?;
                if let Some(min_delay) = constrain.min {
                    writeln!(csv_file, "{:.3}", min_delay)?;
                } else {
                    writeln!(csv_file)?;
                }
            }
        }
    }

    let mut out_file = BufWriter::new(fs::File::create(sdc)?);

    writeln!(
        out_file,
        "create_clock -period {:.3} [get_port clk]",
        constraints.pseudoclock_period
    )?;

    sdc::write_path_constraints(
        &mut out_file,
        &constraints.path_constraints,
        constraints.pseudoclock_period,
    )?;

    if let Some(output) = vcd {
        let mut out_file = BufWriter::new(fs::File::create(output)?);

        crate::analyse::vcd::write_vcd(&constraints.hbcn, &mut out_file)?;
    }

    if let Some(output) = rpt {
        let mut out_file = BufWriter::new(fs::File::create(output)?);

        let mut cycles = crate::analyse::hbcn::find_critical_cycles(&constraints.hbcn)
            .into_par_iter()
            .map(|cycle| {
                let slack: f64 = cycle
                    .iter()
                    .map(|(is, it)| {
                        let ie = constraints.hbcn.find_edge(*is, *it).unwrap();
                        let e = &constraints.hbcn[ie];

                        e.slack()
                    })
                    .sum();
                (slack, cycle)
            })
            .collect::<Vec<_>>();
        writeln!(out_file, "Cycle time constraint: {:.3} ns", cycle_time,)?;
        writeln!(out_file, "Cycles: {}", cycles.len())?;
        cycles.par_sort_unstable_by_key(|(slack, _)| OrderedFloat(*slack));

        for (i, (slack, cycle)) in cycles.into_iter().enumerate() {
            let mut table = Table::new();
            let count = cycle.len();
            let mut tokens = 0;
            table.set_titles(row![
                "T",
                "Node",
                "Transition",
                "Cost",
                "Min Delay",
                "Max Delay",
                "Slack",
                "Time",
            ]);
            table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
            for (is, it) in cycle {
                let ie = constraints.hbcn.find_edge(is, it).unwrap();
                let e = &constraints.hbcn[ie];
                let s = &constraints.hbcn[is];
                let t = &constraints.hbcn[it];

                let slack = e.slack.unwrap_or(0.0);
                let vdelay = e.weight();

                let ttype = match (&s.transition, &t.transition) {
                    (Transition::Data(_), Transition::Data(_)) => "Data Prop",
                    (Transition::Spacer(_), Transition::Spacer(_)) => "Null Prop",
                    (Transition::Data(_), Transition::Spacer(_)) => "Data Ack",
                    (Transition::Spacer(_), Transition::Data(_)) => "Null Ack",
                };

                let min_delay = e.delay.min.unwrap_or(0.0);
                let max_delay = e.delay.max;

                table.add_row(row![
                    if e.is_marked() {
                        tokens += 1;
                        "*"
                    } else {
                        " "
                    },
                    s.name(),
                    ttype,
                    format!("{:.3}", vdelay),
                    format!("{:.3}", min_delay),
                    format!("{:.3}", max_delay),
                    format!("{:.3}", slack),
                    format!("{:.3}", s.time),
                ]);
            }

            writeln!(
                out_file,
                "\nCycle {}: total slack = {:.3} ns ({} transitions / {} {})",
                i,
                slack,
                count,
                tokens,
                if tokens == 1 { "token" } else { "tokens" }
            )?;
            table.print(&mut out_file)?;
        }
    }

    Ok(())
}
