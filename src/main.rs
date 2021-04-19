mod hbcn;
mod sdc;
//mod slack_match;
mod structural_graph;

use clap;
use gag::Gag;
use hbcn::Transition;
use petgraph::dot;
use prettytable::*;
use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    io::{BufWriter, Write},
};
use structural_graph::CircuitNode;

#[derive(Debug, PartialEq, Eq)]
pub enum SolverError {
    Infeasible,
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Problem Infeasible")
    }
}

impl Error for SolverError {}

fn read_file(file_name: &str) -> Result<structural_graph::StructuralGraph, Box<dyn Error>> {
    let file = fs::read_to_string(file_name)?;
    Ok(structural_graph::parse(&file)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let main_args = clap::App::new("HBCN Tools")
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about("Pulsar HBCN timing analysis tools")
        .arg(
            clap::Arg::with_name("input")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .setting(clap::AppSettings::SubcommandRequired)
        .subcommand(
            clap::SubCommand::with_name("depth").about("Find the minimal pseudo-clock divisor"),
        )
        .subcommand(
            clap::SubCommand::with_name("analyse")
                .about("Compute the virtual cycle time.")
                .arg(
                    clap::Arg::with_name("dot")
                        .long("dot")
                        .value_name("dot file"),
                )
                .arg(
                    clap::Arg::with_name("vcd")
                        .long("vcd")
                        .value_name("vcd file"),
                ),
        )
        .subcommand(
            clap::SubCommand::with_name("constrain")
                .about("Create a multi-path SDC file to constrain the circuit cycle time")
                .arg(
                    clap::Arg::with_name("reflexive")
                        .short("r")
                        .help("Constraint Reflexive paths for WINDS")
                        .takes_value(false)
                        .multiple(false),
                )
                .arg(
                    clap::Arg::with_name("divisor")
                        .short("d")
                        .help("pseudo-clock/cycle-time divisor")
                        .required(true)
                        .takes_value(true)
                        .value_name("clock divisor"),
                )
                .arg(
                    clap::Arg::with_name("output")
                        .short("o")
                        .help("Output SDC file")
                        .required(true)
                        .takes_value(true)
                        .value_name("sdc file"),
                )
                .arg(
                    clap::Arg::with_name("csv")
                        .long("csv")
                        .help("Output CSV file")
                        .takes_value(true)
                        .value_name("csv file"),
                ),
        )
        .get_matches();
    let g = read_file(main_args.value_of("input").unwrap())?;

    match main_args.subcommand() {
        //("lint", Some(args)) => lint_main(&g, args),
        ("analyse", Some(args)) => analyse_main(&g, args),
        ("constrain", Some(args)) => constrain_main(&g, args),
        ("depth", _) => depth_main(&g),
        (x, _) => panic!("Subcommand {} not handled", x),
    }
}

fn depth_main(g: &structural_graph::StructuralGraph) -> Result<(), Box<dyn Error>> {
    let hbcn = hbcn::from_structural_graph(g, false).unwrap();

    let cycles = hbcn::find_cycles(&hbcn, false);

    if let Some((deepest, _)) = cycles.first() {
        println!("Greatest Cycle Depth (Mininal Divisor Value): {}", deepest);
    }

    for (i, (cost, cycle)) in cycles.into_iter().enumerate() {
        println!("\nCycle {} ({}):", i, cost);
        let mut table = Table::new();
        table.set_titles(row!["Source", "Target", "Type", "vDelay"]);
        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

        for (is, it) in cycle.into_iter() {
            let ie = hbcn.find_edge(is, it).unwrap();
            let ref s = hbcn[is];
            let ref t = hbcn[it];
            let ref e = hbcn[ie];

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
    g: &structural_graph::StructuralGraph,
    args: &clap::ArgMatches,
) -> Result<(), Box<dyn Error>> {
    let divisor: u64 = args.value_of("divisor").unwrap().parse()?;
    let reflexive = args.is_present("reflexive");

    let hbcn = hbcn::from_structural_graph(g, reflexive).unwrap();

    let paths = {
        let _gag_stdout = Gag::stdout();
        hbcn::constraint_cycle_time(&hbcn, divisor)
    }?;

    let mut out_file = BufWriter::new(fs::File::create(args.value_of("output").unwrap())?);

    writeln!(
        out_file,
        "create_clock -period [expr ${{PERIOD}} / {}.0] [get_port {{clk}}]",
        divisor
    )?;
    sdc::write_path_constraints(&mut out_file, &paths)?;

    if args.is_present("csv") {
        let mut csv_file = BufWriter::new(fs::File::create(args.value_of("csv").unwrap())?);
        let cost_map: HashMap<(CircuitNode, CircuitNode), u64> = hbcn
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
    g: &structural_graph::StructuralGraph,
    args: &clap::ArgMatches,
) -> Result<(), Box<dyn Error>> {
    let hbcn = hbcn::from_structural_graph(g, false).unwrap();

    let (ct, solved_hbcn) = {
        let _gag_stdout = Gag::stdout();
        hbcn::compute_cycle_time(&hbcn)
    }?;

    println!("Worst Virtual Cycletime: {} ps", ct);

    if args.is_present("dot") {
        let filename = args.value_of("dot").unwrap();
        fs::write(filename, format!("{:?}", dot::Dot::new(&solved_hbcn)))?;
    }

    if args.is_present("vcd") {
        let filename = args.value_of("vcd").unwrap();
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
            let ref s = solved_hbcn[is];
            let ref t = solved_hbcn[it];
            let ref e = solved_hbcn[ie];

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

//fn lint_main(
//    g: &structural_graph::StructuralGraph,
//    args: &clap::ArgMatches,
//) -> Result<(), Box<dyn Error>> {
//    let matched = slack_match::slack_match(g, 0.4, 4.)?;
//
//    if args.is_present("dot") {
//        let filename = args.value_of("dot").unwrap();
//        fs::write(filename, format!("{:?}", dot::Dot::new(&matched)))?;
//    }
//
//    let removals = matched.node_indices().filter_map(|ix| {
//        let (ref name, _, _, remove) = matched[ix];
//        if remove {
//            Some(name)
//        } else {
//            None
//        }
//    });
//
//    let insertions = matched.edge_indices().filter_map(|ix| {
//        let (_, n) = matched[ix];
//        if n > 0 {
//            let (is, id) = matched.edge_endpoints(ix)?;
//            let (ref sname, _, _, _) = matched[is];
//            let (ref dname, _, _, _) = matched[id];
//            Some((sname, dname, n))
//        } else {
//            None
//        }
//    });
//
//    for x in removals {
//        println!("Remove {} ", x);
//    }
//
//    for (s, d, n) in insertions {
//        println!("Insert {} buffers between {} and {}", n, s, d);
//    }
//
//    Ok(())
//}
