mod hbcn;
mod sdc;
mod structural_graph;

use gag::Gag;
use hbcn::Transition;
use petgraph::dot;
use prettytable::*;
use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};
use structopt::StructOpt;
use structural_graph::CircuitNode;

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

#[derive(Debug, StructOpt)]
#[structopt(name = "HBCN Tools", about = "Pulsar HBCN timing analysis tools")]
enum CLIArguments {
    /// Find longest path depth in the HBCN, it can be used to define the minimal zeta.
    Depth {
        /// Structural graph input file
        #[structopt(parse(from_os_str))]
        input: PathBuf,
    },
    /// Estimate the virtual-delay cycle-time, it can be used to tune the circuit performance.
    Analyse {
        #[structopt(parse(from_os_str))]
        /// Structural graph input file
        input: PathBuf,

        /// VCD waveform file with virtual-delay arrival times
        #[structopt(long, parse(from_os_str))]
        vcd: Option<PathBuf>,

        /// DOT file displaying the HBCN marked graph
        #[structopt(long, parse(from_os_str))]
        dot: Option<PathBuf>,
    },
    /// Constrain the cycle-time using quantised steps
    ConstrainQuantised {
        /// Structural graph input file
        #[structopt(parse(from_os_str))]
        input: PathBuf,

        /// Cycle-time divisor factor zeta
        #[structopt(short, long)]
        zeta: Option<usize>,

        /// Output SDC constraints file
        #[structopt(long, parse(from_os_str), required_unless("csv"))]
        sdc: Option<PathBuf>,

        /// Output CSV file comprising the constraints
        #[structopt(long, parse(from_os_str), required_unless("sdc"))]
        csv: Option<PathBuf>,

        /// Enable reflexive paths for WInDS
        #[structopt(short, long)]
        reflexive_paths: bool,
    },
    /// Constrain the cycle-time using continous proportional constraints.
    Constrain {
        /// Structural graph input file
        #[structopt(parse(from_os_str))]
        input: PathBuf,

        /// Cycle-time constraint in nanoseconds
        #[structopt(short("t"), long)]
        cycle_time: f64,

        /// Minimal propagation-path delay in nanoseconds
        #[structopt(short, long)]
        minimal_delay: f64,

        /// Output SDC constraints file
        #[structopt(long, parse(from_os_str), required_unless("csv"))]
        sdc: Option<PathBuf>,

        /// Output CSV file comprising the constraints
        #[structopt(long, parse(from_os_str), required_unless("sdc"))]
        csv: Option<PathBuf>,

        /// Enable reflexive paths for WInDS
        #[structopt(short, long)]
        reflexive_paths: bool,
    },
}

fn read_file(file_name: &Path) -> Result<structural_graph::StructuralGraph, Box<dyn Error>> {
    let file = fs::read_to_string(file_name)?;
    Ok(structural_graph::parse(&file)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = CLIArguments::from_args();

    match args {
        CLIArguments::Constrain {
            ref input,
            cycle_time,
            minimal_delay,
            reflexive_paths,
            ref sdc,
            ref csv,
        } => constrain_main(
            input,
            cycle_time,
            minimal_delay,
            reflexive_paths,
            sdc.as_deref(),
            csv.as_deref(),
        ),
        CLIArguments::ConstrainQuantised {
            ref input,
            zeta,
            reflexive_paths,
            ref sdc,
            ref csv,
        } => constrain_quantised_main(input, zeta, reflexive_paths, sdc.as_deref(), csv.as_deref()),
        CLIArguments::Analyse {
            ref input,
            ref dot,
            ref vcd,
        } => analyse_main(input, dot, vcd),
        CLIArguments::Depth { ref input } => depth_main(input),
    }
}

fn depth_main(input: &Path) -> Result<(), Box<dyn Error>> {
    let g = read_file(input)?;
    let hbcn = hbcn::from_structural_graph(&g, false).unwrap();

    let cycles = hbcn::find_cycles(&hbcn, false);

    if let Some((deepest, _)) = cycles.first() {
        println!("Greatest Cycle Depth (Minimal Zeta Value): {}", deepest);
    }

    for (i, (cost, cycle)) in cycles.into_iter().enumerate() {
        println!("\nCycle {} ({}):", i, cost);
        let mut table = Table::new();
        table.set_titles(row!["Source", "Target", "Type", "vDelay"]);
        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

        for (is, it) in cycle.into_iter() {
            let ie = hbcn.find_edge(is, it).unwrap();
            let s = &hbcn[is];
            let t = &hbcn[it];
            let e = &hbcn[ie];

            let ttype = match (s, t) {
                (Transition::Data(_), Transition::Data(_)) => "Data Prop",
                (Transition::Spacer(_), Transition::Spacer(_)) => "Null Prop",
                (Transition::Data(_), Transition::Spacer(_)) => "Data Ack",
                (Transition::Spacer(_), Transition::Data(_)) => "Null Ack",
            };
            table.add_row(row![s.name(), t.name(), ttype, format!("{} ps", e.weight)]);
        }

        table.printstd();
    }

    Ok(())
}

fn constrain_main(
    input: &Path,
    cycle_time: f64,
    minimal_delay: f64,
    reflexive: bool,
    sdc: Option<&Path>,
    csv: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let g = read_file(input)?;

    if let (None, None) = (sdc, csv) {
        return Err("At least one output format must be selected".into());
    }

    let hbcn = hbcn::from_structural_graph(&g, reflexive).unwrap();

    let paths = {
        //let _gag_stdout = Gag::stdout();
        hbcn::constraint_cycle_time(&hbcn, cycle_time, minimal_delay)
    }?;

    if let Some(output) = sdc {
        let mut out_file = BufWriter::new(fs::File::create(output)?);

        writeln!(
            out_file,
            "create_clock -period {:.3} [get_port {{clk}}]",
            minimal_delay
        )?;
        sdc::write_path_constraints(&mut out_file, &paths)?;
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
                "{},{},{:.0},{:.3}",
                src.name(),
                dst.name(),
                cost_map[&(src.clone(), dst.clone())],
                constrain
            )?;
        }
    }

    Ok(())
}

fn constrain_quantised_main(
    input: &Path,
    zeta: Option<usize>,
    reflexive: bool,
    sdc: Option<&Path>,
    csv: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let g = read_file(input)?;

    if let (None, None) = (sdc, csv) {
        return Err("At least one output format must be selected".into());
    }

    let hbcn = hbcn::from_structural_graph(&g, reflexive).unwrap();

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
                constrain
            )?;
        }
    }

    Ok(())
}

fn analyse_main(
    input: &Path,
    dot: &Option<PathBuf>,
    vcd: &Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let g = read_file(input)?;
    let hbcn = hbcn::from_structural_graph(&g, false).unwrap();

    let (ct, solved_hbcn) = {
        let _gag_stdout = Gag::stdout();
        hbcn::compute_cycle_time(&hbcn)
    }?;

    println!("Worst virtual cycle-time: {} ps", ct);

    if let Some(filename) = dot {
        fs::write(filename, format!("{:?}", dot::Dot::new(&solved_hbcn)))?;
    }

    if let Some(filename) = vcd {
        let mut file = std::io::BufWriter::new(fs::File::create(filename)?);
        hbcn::write_vcd(&solved_hbcn, &mut file)?;
    }

    let cycles = hbcn::find_cycles(&hbcn, true);
    for (i, (cost, cycle)) in cycles.into_iter().enumerate() {
        println!("\nCycle {} ({} ps):", i, cost);
        let mut table = Table::new();
        table.set_titles(row![
            "Source", "Target", "Type", "vDelay", "Slack", "Start", "Arrival",
        ]);
        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

        for (is, it) in cycle.into_iter() {
            let ie = hbcn.find_edge(is, it).unwrap();
            let s = &solved_hbcn[is];
            let t = &solved_hbcn[it];
            let e = &solved_hbcn[ie];

            let ttype = match (&s.transition, &t.transition) {
                (Transition::Data(_), Transition::Data(_)) => "Data Prop",
                (Transition::Spacer(_), Transition::Spacer(_)) => "Null Prop",
                (Transition::Data(_), Transition::Spacer(_)) => "Data Ack",
                (Transition::Spacer(_), Transition::Data(_)) => "Null Ack",
            };
            table.add_row(row![
                s.transition.name(),
                t.transition.name(),
                ttype,
                format!("{} ps", e.place.weight),
                format!("{} ps", e.slack),
                format!("{} ps", s.time),
                format!("{} ps", s.time + e.slack + e.place.weight),
            ]);
        }

        table.printstd();
    }

    Ok(())
}
