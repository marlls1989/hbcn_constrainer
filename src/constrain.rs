use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use ordered_float::OrderedFloat;
use prettytable::*;
use rayon::prelude::*;

use crate::{hbcn, hbcn::Transition, read_file, sdc, structural_graph::CircuitNode};

#[derive(Parser, Debug)]
pub struct ConstrainArgs {
    /// Structural graph input file
    #[clap(parse(from_os_str))]
    input: PathBuf,

    /// Output SDC constraints file
    #[clap(long, parse(from_os_str))]
    sdc: PathBuf,

    /// Cycle-time constraint in nanoseconds
    #[clap(short('t'), long)]
    cycle_time: f64,

    /// Minimal propagation-path delay in nanoseconds
    #[clap(short, long)]
    minimal_delay: f64,

    /// Output CSV file
    #[clap(long, parse(from_os_str))]
    csv: Option<PathBuf>,

    /// Output report file
    #[clap(long, parse(from_os_str))]
    rpt: Option<PathBuf>,

    /// Output VCD file with arrival times
    #[clap(long, parse(from_os_str))]
    vcd: Option<PathBuf>,

    /// Enable reflexive paths for WInDS (deprecated)
    #[clap(short, long)]
    reflexive_paths: bool,

    /// Constraint tight self loops (for MouseTrap)
    #[clap(long)]
    tight_self_loops: Option<f64>,

    /// Use pseudo-clock to constrain paths
    #[clap(long)]
    no_proportinal: bool,

    /// Use forward completion delay if greater than path virtual delay
    #[clap(long)]
    forward_completion: bool,

    /// Percentual margin between maximum and minimum delay in the forward path
    #[clap(long, short('f'), value_parser = clap::value_parser!(u8).range(0 .. 100))]
    forward_margin: Option<u8>,

    /// Minimal percentual margin between maximum and minimum delay in the backward path
    #[clap(long, short('b'), value_parser = clap::value_parser!(u8).range(0 .. 100))]
    backward_margin: Option<u8>,
}

pub fn constrain_main(args: ConstrainArgs) -> Result<(), Box<dyn Error>> {
    let ConstrainArgs {
        input,
        cycle_time,
        minimal_delay,
        ref sdc,
        ref csv,
        ref rpt,
        ref vcd,
        reflexive_paths,
        tight_self_loops,
        no_proportinal,
        forward_completion,
        forward_margin,
        backward_margin,
    } = args;
    let g = read_file(&input)?;
    let forward_margin = forward_margin.map(|x| 1.0 - (x as f64 / 100.0));
    let backward_margin = backward_margin.map(|x| 1.0 - (x as f64 / 100.0));

    let hbcn = hbcn::from_structural_graph(&g, reflexive_paths, forward_completion).unwrap();

    let mut constraints = if no_proportinal {
        hbcn::constrain_cycle_time_pseudoclock(&hbcn, cycle_time, minimal_delay)?
    } else {
        hbcn::constrain_cycle_time_proportional(
            &hbcn,
            cycle_time,
            minimal_delay,
            backward_margin,
            forward_margin,
        )?
    };

    if let Some(val) = tight_self_loops {
        hbcn::constrain_selfreflexive_paths(&mut constraints.path_constraints, val);
    }

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
                    hbcn[ie].weight,
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

        let mut cycles = hbcn::find_cycles(&constraints.hbcn)
            .into_par_iter()
            .map(|cycle| {
                let slack: f64 = cycle
                    .iter()
                    .map(|(is, it)| {
                        let ie = constraints.hbcn.find_edge(*is, *it).unwrap();
                        constraints.hbcn[ie].slack.unwrap_or(0.0)
                    })
                    .sum();
                (slack, cycle)
            })
            .collect::<Vec<_>>();
        writeln!(out_file, "Cycles: {}\n", cycles.len())?;
        cycles.par_sort_unstable_by_key(|(slack, _)| OrderedFloat(*slack));

        for (i, (slack, cycle)) in cycles.into_iter().enumerate() {
            writeln!(out_file, "Cycle {} total slack {:.3} ps", i, slack)?;
            let mut table = Table::new();
            table.set_titles(row![
                "Source",
                "Target",
                "Type",
                "vDelay",
                "Min Delay",
                "Max Delay",
                "Slack",
                "Start",
                "Arrival"
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
                    s.transition.name(),
                    t.transition.name(),
                    ttype,
                    format!("{:.3}", vdelay),
                    format!("{:.3}", min_delay),
                    format!("{:.3}", max_delay),
                    format!("{:.3}", slack),
                    format!("{:.3}", s.time),
                    format!("{:.3}", t.time),
                ]);
            }
        }
    }

    Ok(())
}
