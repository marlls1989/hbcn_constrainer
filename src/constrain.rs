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
pub struct ConstrainQuantisedArgs {
    /// Structural graph input file
    #[clap(parse(from_os_str))]
    input: PathBuf,

    /// Cycle-time divisor factor zeta
    #[clap(short, long)]
    zeta: Option<usize>,

    /// Output SDC constraints file
    #[clap(long, parse(from_os_str), required_unless("csv"))]
    sdc: Option<PathBuf>,

    /// Output CSV file comprising the constraints
    #[clap(long, parse(from_os_str), required_unless("sdc"))]
    csv: Option<PathBuf>,

    /// Enable reflexive paths for WInDS
    #[clap(short, long)]
    reflexive_paths: bool,

    /// Use forward completion delay if greater than path virtual delay
    #[clap(long)]
    forward_completion: bool,
}

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

    /// Enable reflexive paths for WInDS
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
    } = args;
    let g = read_file(&input)?;

    if let (None, None) = (sdc, csv) {
        return Err("At least one output format must be selected".into());
    }

    let hbcn = hbcn::from_structural_graph(&g, reflexive_paths, forward_completion).unwrap();

    let (pseudo_clock, mut paths) = if no_proportinal {
        hbcn::constraint_cycle_time_pseudoclock(&hbcn, cycle_time, minimal_delay)?
    } else {
        hbcn::constraint_cycle_time_proportional(&hbcn, cycle_time, minimal_delay)?
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
        writeln!(csv_file, "src,dst,cost,constrain")?;
        for (key, constrain) in paths.iter() {
            if let Some(cost) = cost_map.get(key) {
                let (src, dst) = key;
                writeln!(
                    csv_file,
                    "{},{},{:.0},{:.3}",
                    src.name(),
                    dst.name(),
                    cost,
                    constrain.max
                )?;
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

        let paths: HashMap<_, _> = paths
            .into_iter()
            .filter(|(_k, v)| ((v.max - pseudo_clock).abs() / pseudo_clock) > 0.01)
            .collect();

        sdc::write_path_constraints(&mut out_file, &paths)?;
    }

    Ok(())
}

pub fn constrain_quantised_main(args: ConstrainQuantisedArgs) -> Result<(), Box<dyn Error>> {
    let ConstrainQuantisedArgs {
        input,
        zeta,
        ref sdc,
        ref csv,
        reflexive_paths,
        forward_completion,
    } = args;

    let g = read_file(&input)?;

    if let (None, None) = (sdc, csv) {
        return Err("At least one output format must be selected".into());
    }

    let hbcn = hbcn::from_structural_graph(&g, reflexive_paths, forward_completion).unwrap();

    let zeta = zeta.unwrap_or_else(|| {
        let zeta = hbcn::best_zeta(&hbcn);
        eprintln!("Found zeta value: {}", zeta);
        zeta
    });

    let paths = {
        //let _gag_stdout = Gag::stdout();
        hbcn::constraint_cycle_time_quantised(&hbcn, zeta)
    }?;

    if let Some(output) = sdc {
        let mut out_file = BufWriter::new(fs::File::create(output)?);

        writeln!(
            out_file,
            "create_clock -period [expr ${{PERIOD}} / {}.0] [get_port {{clk}}]",
            zeta
        )?;
        sdc::write_path_quantised_constraints(&mut out_file, &paths)?;
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
        writeln!(csv_file, "src,dst,cost,constrain")?;
        for ((src, dst), constrain) in paths.iter() {
            writeln!(
                csv_file,
                "{},{},{},{}",
                src.name(),
                dst.name(),
                cost_map[&(src.clone(), dst.clone())],
                constrain.max
            )?;
        }
    }

    Ok(())
}
