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

use crate::{
    hbcn::{self, *},
    read_file,
    structural_graph::CircuitNode,
};

mod sdc;
#[cfg(test)]
mod tests;

#[derive(Parser, Debug)]
pub struct ConstrainArgs {
    /// Structural graph input file
    input: PathBuf,

    /// Output SDC constraints file
    #[clap(long)]
    sdc: PathBuf,

    /// Cycle-time constraint
    #[clap(short('t'), long)]
    cycle_time: f64,

    /// Minimal propagation-path delay
    #[clap(short, long)]
    minimal_delay: f64,

    /// Output CSV file
    #[clap(long)]
    csv: Option<PathBuf>,

    /// Output report file
    #[clap(long)]
    rpt: Option<PathBuf>,

    /// Output VCD file with arrival times
    #[clap(long)]
    vcd: Option<PathBuf>,

    /// Use pseudo-clock to constrain paths
    #[clap(long)]
    no_proportinal: bool,

    /// Don't use forward completion delay if greater than path virtual delay
    #[clap(long)]
    no_forward_completion: bool,

    /// Percentual margin between maximum and minimum delay in the forward path
    #[clap(long, short('f'), value_parser = clap::value_parser!(u8).range(0 .. 100))]
    forward_margin: Option<u8>,

    /// Minimal percentual margin between maximum and minimum delay in the backward path
    #[clap(long, short('b'), value_parser = clap::value_parser!(u8).range(0 .. 100))]
    backward_margin: Option<u8>,
}

pub fn constrain_main(args: ConstrainArgs) -> Result<()> {
    let ConstrainArgs {
        input,
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
        let hbcn = {
            let g = read_file(&input)?;
            from_structural_graph(&g, forward_completion)
                .ok_or_else(|| anyhow!("Failed to convert structural graph to StructuralHBCN"))?
        };

        if no_proportinal {
            constrain_cycle_time_pseudoclock(&hbcn, cycle_time, minimal_delay)?
        } else {
            constrain_cycle_time_proportional(
                &hbcn,
                cycle_time,
                minimal_delay,
                backward_margin,
                forward_margin,
            )?
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
                        hbcn[is].circuit_node().clone(),
                        hbcn[id].circuit_node().clone(),
                    ),
                    hbcn[ie].place.weight,
                ))
            })
            .collect();
        writeln!(csv_file, "src,dst,cost,max_delay,min_delay")?;
        for (key, constrain) in constraints.path_constraints.iter() {
            if let Some(cost) = cost_map.get(key) {
                let (src, dst) = key;
                write!(csv_file, "{},{},{:.0},", src.name(), dst.name(), cost,)?;
                if let Some(max_delay) = constrain.max {
                    write!(csv_file, "{:.3},", max_delay)?;
                } else {
                    write!(csv_file, ",")?;
                }
                if let Some(min_delay) = constrain.min {
                    writeln!(csv_file, "{:.3}", min_delay)?;
                } else {
                    writeln!(csv_file, "")?;
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

    sdc::write_path_constraints(&mut out_file, &constraints.path_constraints)?;

    if let Some(output) = vcd {
        let mut out_file = BufWriter::new(fs::File::create(output)?);

        hbcn::write_vcd(&constraints.hbcn, &mut out_file)?;
    }

    if let Some(output) = rpt {
        let mut out_file = BufWriter::new(fs::File::create(output)?);

        let mut cycles = hbcn::find_critical_cycles(&constraints.hbcn)
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
                let vdelay = e.place.weight;

                let ttype = match (&s.transition, &t.transition) {
                    (Transition::Data(_), Transition::Data(_)) => "Data Prop",
                    (Transition::Spacer(_), Transition::Spacer(_)) => "Null Prop",
                    (Transition::Data(_), Transition::Spacer(_)) => "Data Ack",
                    (Transition::Spacer(_), Transition::Data(_)) => "Null Ack",
                };

                let min_delay = e.delay.min.unwrap_or(0.0);
                let max_delay = e.delay.max.unwrap_or(0.0);

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
