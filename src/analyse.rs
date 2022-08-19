use std::{error::Error, fs, path::PathBuf};

use clap::Parser;
use gag::Gag;
use petgraph::dot;
use prettytable::*;

use crate::{
    hbcn::{self, Transition},
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
    let g = read_file(&input)?;
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

pub fn depth_main(args: DepthArgs) -> Result<(), Box<dyn Error>> {
    let DepthArgs { input } = args;
    let g = read_file(&input)?;
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
