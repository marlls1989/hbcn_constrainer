mod hbcn;
mod sdc;
//mod slack_match;
mod structural_graph;

use clap;
use petgraph::dot;
use std::{
    collections::BinaryHeap,
    error::Error,
    fmt, fs,
    io::{BufWriter, Write},
};

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
        //        .subcommand(
        //            clap::SubCommand::with_name("lint")
        //                .about("Perform slack matching to advise on buffer insertion and removal.")
        //                .arg(
        //                    clap::Arg::with_name("dot")
        //                        .long("dot")
        //                        .value_name("DOT FILE"),
        //                ),
        //        )
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
                )
                .arg(
                    clap::Arg::with_name("reflexive")
                        .short("r")
                        .takes_value(false)
                        .help("Reflexive Paths"),
                ),
        )
        .subcommand(
            clap::SubCommand::with_name("constrain")
                .about("Create a multi-path SDC file to constrain the circuit cycle time")
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
                    clap::Arg::with_name("reflexive")
                        .short("r")
                        .takes_value(false)
                        .help("Reflexive Paths"),
                ),
        )
        .get_matches();
    let g = read_file(main_args.value_of("input").unwrap())?;

    match main_args.subcommand() {
        //("lint", Some(args)) => lint_main(&g, args),
        ("analyse", Some(args)) => analyse_main(&g, args),
        ("constrain", Some(args)) => constrain_main(&g, args),
        (x, _) => panic!("Subcommand {} not handled", x),
    }
}

fn constrain_main(
    g: &structural_graph::StructuralGraph,
    args: &clap::ArgMatches,
) -> Result<(), Box<dyn Error>> {
    let divisor: u64 = args.value_of("divisor").unwrap().parse()?;
    let reflexive = args.is_present("reflexive");
    let mut out_file = BufWriter::new(fs::File::create(args.value_of("output").unwrap())?);

    let hbcn = hbcn::from_structural_graph(g, reflexive).unwrap();
    let paths = hbcn::constraint_cycle_time(&hbcn, divisor)?;

    writeln!(
        out_file,
        "create_clock -period [expr ${{PERIOD}} / {}.0] [get_port {{clk}}]",
        divisor
    )?;
    sdc::write_path_constraints(&mut out_file, &paths)?;

    Ok(())
}

fn analyse_main(
    g: &structural_graph::StructuralGraph,
    args: &clap::ArgMatches,
) -> Result<(), Box<dyn Error>> {
    let reflexive = args.is_present("reflexive");
    let hbcn = hbcn::from_structural_graph(g, reflexive).unwrap();
    let (ct, hbcn) = hbcn::compute_cycle_time(&hbcn)?;

    let mut slack: BinaryHeap<_> = hbcn
        .edge_indices()
        .map(|ie| {
            let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
            (hbcn[ie].slack, &hbcn[src].transition, &hbcn[dst].transition)
        })
        .collect();

    while let Some((slack, src, dst)) = slack.pop() {
        println!("{}ps of free slack on ({} -> {})", slack, src, dst);
    }

    println!("Cycletime: {}ps", ct);

    if args.is_present("dot") {
        let filename = args.value_of("dot").unwrap();
        fs::write(filename, format!("{:?}", dot::Dot::new(&hbcn)))?;
    }

    if args.is_present("vcd") {
        let filename = args.value_of("vcd").unwrap();
        let mut file = std::io::BufWriter::new(fs::File::create(filename)?);
        hbcn::write_vcd(&hbcn, &mut file)?;
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
