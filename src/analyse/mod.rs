use std::{cmp, fs, path::PathBuf};

use anyhow::*;
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
    #[clap(value_parser = clap::value_parser!(std::path::PathBuf))]
    /// Structural graph input file
    input: PathBuf,

    /// VCD waveform file with virtual-delay arrival times
    #[clap(long, value_parser = clap::value_parser!(std::path::PathBuf))]
    vcd: Option<PathBuf>,

    /// DOT file displaying the HBCN marked graph
    #[clap(long, value_parser = clap::value_parser!(std::path::PathBuf))]
    dot: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct DepthArgs {
    /// Structural graph input file
    #[clap(value_parser = clap::value_parser!(std::path::PathBuf))]
    input: PathBuf,
}

pub fn analyse_main(args: AnalyseArgs) -> Result<()> {
    let AnalyseArgs { input, vcd, dot } = args;

    let (ct, solved_hbcn) = {
        let g = read_file(&input)?;
        let hbcn = hbcn::from_structural_graph(&g, false, false)
            .ok_or_else(|| anyhow!("Failed to convert structural graph to HBCN"))?;
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
        let count = cycle.len();
        table.set_titles(row![
            "T",
            "Node",
            "Transition",
            "Cost",
            "Slack",
            "Delay",
            "Time",
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
                    " "
                },
                s.name(),
                ttype,
                format!("{}", e.place.weight()),
                format!("{}", e.slack()),
                format!("{}", e.weight()),
                format!("{}", s.time),
            ]);
        }

        println!(
            "\nCycle {}: cost - slack = {} ({} transitions / {} {}):",
            i,
            cost,
            count,
            tokens,
            if tokens == 1 { "token" } else { "tokens" }
        );
        table.printstd();
    }

    Ok(())
}

pub fn depth_main(args: DepthArgs) -> Result<()> {
    let DepthArgs { input } = args;

    let (ct, solved_hbcn) = {
        let g = read_file(&input)?;
        let hbcn = hbcn::from_structural_graph(&g, false, false)
            .ok_or_else(|| anyhow!("Failed to convert structural graph to HBCN"))?;
        let _gag_stdout = Gag::stdout();
        hbcn::compute_cycle_time(&hbcn, false)
    }?;

    println!("Critical Cycle (Depth/Tokens): {}", ct);

    let mut cycles = hbcn::find_critical_cycles(&solved_hbcn);

    cycles.par_sort_unstable_by_key(|cycle| cmp::Reverse(cycle.len()));

    for (i, cycle) in cycles.into_iter().enumerate() {
        let cost = cycle.len();
        let mut table = Table::new();
        let count = cycle.len();
        let mut tokens = 0;
        table.set_titles(row!["T", "Node", "Transition", "Slack", "Time"]);
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
                ttype,
                format!("{}", e.slack() as usize),
                format!("{}", s.time),
            ]);
        }

        println!(
            "\nCycle {}: total cost = {} ({} transitions / {} {}):",
            i,
            cost,
            count,
            tokens,
            if tokens == 1 { "token" } else { "tokens" }
        );
        table.printstd();
    }

    Ok(())
}
