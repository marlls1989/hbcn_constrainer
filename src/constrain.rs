use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;

use crate::{hbcn, read_file, sdc, structural_graph::CircuitNode};

#[derive(Parser, Debug)]
pub struct ConstrainArgs {
    /// Structural graph input file
    #[clap(parse(from_os_str))]
    input: PathBuf,

    /// Cycle-time constraint in nanoseconds
    #[clap(short('t'), long)]
    cycle_time: f64,

    /// Minimal propagation-path delay in nanoseconds
    #[clap(short, long)]
    minimal_delay: f64,

    /// Output SDC constraints file
    #[clap(long, parse(from_os_str), required_unless_present("csv"))]
    sdc: Option<PathBuf>,

    /// Output CSV file comprising the constraints
    #[clap(long, parse(from_os_str), required_unless_present("sdc"))]
    csv: Option<PathBuf>,

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
        reflexive_paths,
        tight_self_loops,
        no_proportinal,
        forward_completion,
        forward_margin,
        backward_margin,
    } = args;
    let g = read_file(&input)?;
    let forward_margin = forward_margin.map(|x| x as f64 / 100.0);
    let backward_margin = backward_margin.map(|x| x as f64 / 100.0);

    if let (None, None) = (sdc, csv) {
        return Err("At least one output format must be selected".into());
    }

    let hbcn = hbcn::from_structural_graph(&g, reflexive_paths, forward_completion).unwrap();

    let (pseudo_clock, mut paths) = if no_proportinal {
        hbcn::constraint_cycle_time_pseudoclock(&hbcn, cycle_time, minimal_delay)?
    } else {
        hbcn::constraint_cycle_time_proportional(
            &hbcn,
            cycle_time,
            minimal_delay,
            backward_margin,
            forward_margin,
        )?
    };

    if let Some(val) = tight_self_loops {
        hbcn::constraint_selfreflexive_paths(&mut paths, val);
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
        for (key, constrain) in paths.iter() {
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

    if let Some(output) = sdc {
        let mut out_file = BufWriter::new(fs::File::create(output)?);

        writeln!(
            out_file,
            "create_clock -period {:.3} [get_port clk]",
            pseudo_clock
        )?;

        sdc::write_path_constraints(&mut out_file, &paths)?;
    }

    Ok(())
}
