//! VCD (Value Change Dump) waveform generation for HBCN timing visualization.
//!
//! This module provides functionality to generate VCD files that can be viewed in waveform
//! viewers (like GTKWave) to visualize the timing behavior of HBCN circuits.
//!
//! # Format
//!
//! The generated VCD file follows the standard VCD format and includes:
//!
//! - Timescale information (picoseconds)
//! - Variable declarations for each circuit node
//! - Value changes showing data vs. spacer transitions over time
//!
//! # Usage
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use hbcn::analyse::vcd::write_vcd;
//! use std::io::BufWriter;
//! use std::fs::File;
//! # let solved_hbcn = hbcn::hbcn::SolvedHBCN::default(); // Example only
//!
//! let mut output = BufWriter::new(File::create("timing.vcd")?);
//! write_vcd(&solved_hbcn, &mut output)?;
//! # Ok(())
//! # }
//! ```

use std::{collections::HashMap, io};

use anyhow::Result;
use itertools::Itertools;
use petgraph::visit::IntoNodeReferences;
use rayon::prelude::*;
use regex::Regex;

use crate::hbcn::{HBCN, HasTransition, Named, TimedEvent, Transition};

/// Write VCD (Value Change Dump) format output for an HBCN.
///
/// This function generates a VCD file that represents the timing behavior of the
/// circuit. Each circuit node is represented as a wire, with transitions occurring
/// at their computed times:
///
/// - **Data transitions** → wire value = 1
/// - **Spacer transitions** → wire value = 0
///
/// # Arguments
///
/// * `hbcn` - The solved HBCN with timing information
/// * `w` - Writer to output the VCD file to
///
/// # Example
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use hbcn::analyse::vcd::write_vcd;
/// use std::io::BufWriter;
/// use std::fs::File;
/// # let solved_hbcn = hbcn::hbcn::SolvedHBCN::default(); // Example only
///
/// let mut file = BufWriter::new(File::create("circuit.vcd")?);
/// write_vcd(&solved_hbcn, &mut file)?;
/// # Ok(())
/// # }
/// ```
pub fn write_vcd<T: HasTransition + TimedEvent + Send + Sync, P>(
    hbcn: &HBCN<T, P>,
    w: &mut dyn io::Write,
) -> Result<()> {
    let mut writer = vcd::Writer::new(w);
    let re = Regex::new(r"[^a-zA-Z0-9_]").unwrap();

    writer.timescale(1, vcd::TimescaleUnit::PS)?;
    writer.add_module("top")?;

    let mut variables = HashMap::new();

    let events = {
        let mut events: Vec<&T> = hbcn
            .node_references()
            .map(|(_idx, e)| {
                let cnode = e.transition().name();
                if !variables.contains_key(cnode) {
                    variables.insert(
                        cnode.clone(),
                        writer.add_wire(1, &re.replace_all(cnode, "_")).unwrap(),
                    );
                }

                e
            })
            .collect();
        events.par_sort_unstable_by(|a, b| a.time().partial_cmp(&b.time()).unwrap());
        events
    };

    for (_, var) in variables.iter() {
        writer.change_scalar(*var, vcd::Value::V0)?;
    }

    for (time, events) in events.into_iter().group_by(|x| x.time()).into_iter() {
        writer.timestamp((time.abs() * 1000.0) as u64)?;
        for event in events {
            match event.transition() {
                Transition::Data(id) => writer.change_scalar(variables[id.name()], vcd::Value::V1),
                Transition::Spacer(id) => {
                    writer.change_scalar(variables[id.name()], vcd::Value::V0)
                }
            }?;
        }
    }

    Ok(())
}
