mod hbcn;
mod structural_graph;

use clap;
use std::{error::Error, fs};

fn read_file(file_name: &str) -> Result<structural_graph::StructuralGraph, Box<dyn Error>> {
    let file = fs::read_to_string(file_name)?;
    Ok(structural_graph::parse(&file)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let main_args = clap::App::new("HBCN Constrainer")
        .version("0.1.0")
        .author("Marcos Sartori <marcos.sartori@acad.pucrs.br>")
        .about("Pulsar HBCN analysis timing tools")
        .setting(clap::AppSettings::SubcommandRequired)
        .subcommand(
            clap::SubCommand::with_name("lint")
            .about("Perform slack matching analsis to advise on buffer insertion and removal.")
        )
        .arg(
            clap::Arg::with_name("input")
                .help("Sets the input file to use")
                .required(true)
                //.index(1),
        )
        .get_matches();
    let g = read_file(main_args.value_of("input").unwrap())?;

    return match main_args.subcommand() {
        ("lint", Some(_)) => lint_main(&g),
        (x, _) => panic!("Subcommand {} not handled", x),
    }
}

fn lint_main(g: &structural_graph::StructuralGraph) -> Result<(), Box<dyn Error>> {

    let matched = structural_graph::slack_match(g, 0.4, 4.)?;

    let removals = matched.node_indices().filter_map(|ix| {
        let (ref name, _, _, remove) = matched[ix];
        if remove {
            Some(name)
        } else {
           None
        }
    });

    let insertions = matched.edge_indices().filter_map(|ix| {
        let (_, n) = matched[ix];
        if n > 0 {
            let (is, id) = matched.edge_endpoints(ix)?;
            let (ref sname, _, _, _) = matched[is];
            let (ref dname, _, _, _) = matched[id];
            Some((sname, dname, n))
        } else {
            None
        }
    });

    for x in removals {
        println!("Remove {} ", x);
    }

    for (s, d, n) in insertions {
        println!("Insert {} buffers between {} and {}", n, s, d);
    }

    Ok(())
}


