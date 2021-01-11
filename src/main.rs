mod hbcn;
mod structural_graph;

use clap;
use petgraph::dot::Dot;
use std::fs;

fn main() {
    let matches = clap::App::new("HBCN Constrainer")
        .version("0.1.0")
        .author("Marcos Sartori <marcos.sartori@acad.pucrs.br>")
        .about("Pulsar HBCN cycletime constrainer")
        .arg(
            clap::Arg::with_name("input")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .get_matches();
    let file = fs::read_to_string(matches.value_of("input").unwrap()).unwrap();
    let g = structural_graph::parse(&file).unwrap();
    let sm = structural_graph::slack_match(&g, 0.25, 4.).unwrap();
    // let hbcn = hbcn::from_structural_graph(&g, 0.2).unwrap();
    // let hbcn = hbcn::constraint_cycle_time(&hbcn, 4.).unwrap_or(hbcn);

    println!("{:?}", Dot::new(&sm));
}
