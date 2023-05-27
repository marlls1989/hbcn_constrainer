use std::{cmp, error::Error, fs, path::PathBuf};

use clap::Parser;
use gag::Gag;
use ordered_float::OrderedFloat;
use petgraph::dot;
use prettytable::*;
use rayon::prelude::*;

use crate::{
    hbcn::{self, *},
    read_file,
};

#[derive(Parser, Debug)]
pub struct AnalyseArgs {
    #[clap(parse(from_os_str))]
    /// Structural graph input file
    input: PathBuf,

    /// VCD waveform file with virtual-delay arrival times
    #[clap(long, parse(from_os_str))]
    vcd: Option<PathBuf>,

    /// DOT file displaying the HBCN marked graph
    #[clap(long, parse(from_os_str))]
    dot: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct DepthArgs {
    /// Structural graph input file
    #[clap(parse(from_os_str))]
    input: PathBuf,
}

pub fn analyse_main(args: AnalyseArgs) -> Result<(), Box<dyn Error>> {
    let AnalyseArgs { input, vcd, dot } = args;

    let (ct, solved_hbcn) = {
        let g = read_file(&input)?;
        let hbcn = hbcn::from_structural_graph(&g, false, false).unwrap();
        let _gag_stdout = Gag::stdout();
        hbcn::compute_cycle_time(&hbcn, true)
    }?;

    println!("Worst virtual cycle-time: {}", ct);

    if let Some(filename) = dot {
        fs::write(filename, format!("{:?}", dot::Dot::new(&solved_hbcn)))?;
    }

    if let Some(filename) = vcd {
        let mut file = std::io::BufWriter::new(fs::File::create(filename)?);
        hbcn::write_vcd(&solved_hbcn, &mut file)?;
    }

    let mut cycles = hbcn::find_critical_cycles(&solved_hbcn)
        .into_par_iter()
        .map(|cycle| {
            let cost: f64 = cycle
                .iter()
                .map(|(is, it)| {
                    let ie = solved_hbcn.find_edge(*is, *it).unwrap();
                    let e = &solved_hbcn[ie];
                    e.weight() - e.slack()
                })
                .sum();
            (cost, cycle)
        })
        .collect::<Vec<_>>();

    cycles.par_sort_unstable_by_key(|(cost, _)| cmp::Reverse(OrderedFloat(*cost)));

    for (i, (cost, cycle)) in cycles.into_iter().enumerate() {
        let mut table = Table::new();
        let mut tokens = 0;
        table.set_titles(row![
            "T", "Source", "Target", "Type", "Cost", "Slack", "Delay", "Start", "Arrival",
        ]);
        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

        for (is, it) in cycle.into_iter() {
            let ie = solved_hbcn.find_edge(is, it).unwrap();
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
                if e.is_marked() {
                    tokens += 1;
                    "*"
                } else {
                    ""
                },
                s.transition.name(),
                t.transition.name(),
                ttype,
                format!("{}", e.place.weight),
                format!("{}", e.slack()),
                format!("{}", e.delay.max.unwrap_or(0.0)),
                format!("{}", s.time),
                format!("{}", t.time),
            ]);
        }

        println!("\nCycle {} {} (total cost) / {} (tokens):", i, cost, tokens);
        table.printstd();
    }

    Ok(())
}

pub fn depth_main(args: DepthArgs) -> Result<(), Box<dyn Error>> {
    let DepthArgs { input } = args;

    let (ct, solved_hbcn) = {
        let g = read_file(&input)?;
        let hbcn = hbcn::from_structural_graph(&g, false, false).unwrap();
        let _gag_stdout = Gag::stdout();
        hbcn::compute_cycle_time(&hbcn, false)
    }?;

    println!("Critical Cycle (Depth/Tokens): {}", ct);

    let mut cycles = hbcn::find_critical_cycles(&solved_hbcn);

    cycles.par_sort_unstable_by_key(|cycle| cmp::Reverse(cycle.len()));

    for (i, cycle) in cycles.into_iter().enumerate() {
        let cost = cycle.len();
        let mut table = Table::new();
        let mut tokens = 0;
        table.set_titles(row!["T", "Source", "Target", "Type", "Slack"]);
        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

        for (is, it) in cycle.into_iter() {
            let ie = solved_hbcn.find_edge(is, it).unwrap();
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
                if e.is_marked() {
                    tokens += 1;
                    "*"
                } else {
                    " "
                },
                s.transition.name(),
                t.transition.name(),
                ttype,
                format!("{}", e.slack() as usize)
            ]);
        }

        println!("\nCycle {} ({}/{}):", i, cost, tokens);
        table.printstd();
    }

    Ok(())
}
