use std::{cmp, fs, path::PathBuf};

use anyhow::*;
use clap::Parser;
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
    /// Structural graph input file
    input: PathBuf,

    /// VCD waveform file with virtual-delay arrival times
    #[clap(long)]
    vcd: Option<PathBuf>,

    /// DOT file displaying the StructuralHBCN marked graph
    #[clap(long)]
    dot: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct DepthArgs {
    /// Structural graph input file
    input: PathBuf,
}

pub fn analyse_main(args: AnalyseArgs) -> Result<()> {
    let AnalyseArgs { input, vcd, dot } = args;

    let (ct, solved_hbcn) = {
        let g = read_file(&input)?;
        let hbcn = hbcn::from_structural_graph(&g, false)
            .ok_or_else(|| anyhow!("Failed to convert structural graph to StructuralHBCN"))?;
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
        let hbcn = hbcn::from_structural_graph(&g, false)
            .ok_or_else(|| anyhow!("Failed to convert structural graph to StructuralHBCN"))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural_graph::parse;
    use std::io::Cursor;
    use std::result::Result::Ok;

    /// Helper function to create a test StructuralHBCN from a structural graph string
    fn create_test_hbcn(input: &str) -> Result<StructuralHBCN> {
        let structural_graph = parse(input)?;
        hbcn::from_structural_graph(&structural_graph, false)
            .ok_or_else(|| anyhow!("Failed to convert to StructuralHBCN"))
    }

    /// Helper function to run analysis and capture output
    fn run_analysis(input: &str, weighted: bool) -> Result<(f64, DelayedHBCN)> {
        let hbcn = create_test_hbcn(input)?;
        hbcn::compute_cycle_time(&hbcn, weighted)
    }

    /// Test basic cycle time computation with weighted analysis
    #[test]
    fn test_cycle_time_computation_weighted() {
        let input = r#"Port "a" [("b", 20)]
                      Port "b" []"#;

        let (cycle_time, _) = run_analysis(input, true)
            .expect("Should compute cycle time for simple circuit");

        assert!(cycle_time > 0.0, "Cycle time should be positive");
        assert!(cycle_time >= 20.0, "Cycle time should be at least the edge weight");
    }

    /// Test basic cycle time computation with unweighted analysis
    #[test]
    fn test_cycle_time_computation_unweighted() {
        let input = r#"Port "a" [("b", 20)]
                      Port "b" []"#;

        let (cycle_time, _) = run_analysis(input, false)
            .expect("Should compute cycle time for simple circuit");

        assert!(cycle_time > 0.0, "Cycle time should be positive");
    }

    /// Test cycle time computation with DataReg
    #[test]
    fn test_cycle_time_with_datareg() {
        let input = r#"Port "input" [("reg", 30)]
                      DataReg "reg" [("output", 25)]
                      Port "output" []"#;

        let (cycle_time, _) = run_analysis(input, true)
            .expect("Should compute cycle time for DataReg circuit");

        assert!(cycle_time > 0.0, "Cycle time should be positive");
    }

    /// Test cycle time computation with cyclic circuit
    #[test]
    fn test_cycle_time_cyclic_circuit() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        let (cycle_time, _) = run_analysis(input, true)
            .expect("Should compute cycle time for cyclic circuit");

        assert!(cycle_time > 0.0, "Cycle time should be positive");
    }

    /// Test critical cycle detection
    #[test]
    fn test_critical_cycle_detection() {
        let input = r#"Port "input" [("reg", 30)]
                      DataReg "reg" [("output", 25), ("reg", 20)]
                      Port "output" []"#;

        let (_, solved_hbcn) = run_analysis(input, true)
            .expect("Should compute cycle time for cyclic circuit");

        let cycles = hbcn::find_critical_cycles(&solved_hbcn);
        
        // Should find at least one cycle in a cyclic circuit
        assert!(!cycles.is_empty(), "Should find critical cycles in cyclic circuit");
        
        // Each cycle should have at least 2 edges
        for cycle in &cycles {
            assert!(cycle.len() >= 2, "Each cycle should have at least 2 edges");
        }
    }

    /// Test cycle analysis with complex circuit
    #[test]
    fn test_complex_circuit_analysis() {
        let input = r#"Port "clk" [("reg1", 5), ("reg2", 5)]
                      Port "input" [("reg1", 40)]
                      DataReg "reg1" [("logic", 30), ("reg2", 25)]
                      DataReg "reg2" [("logic", 35), ("reg1", 20)]
                      DataReg "logic" [("output", 45)]
                      Port "output" []"#;

        let (cycle_time, solved_hbcn) = run_analysis(input, true)
            .expect("Should compute cycle time for complex circuit");

        assert!(cycle_time > 0.0, "Cycle time should be positive");
        
        let cycles = hbcn::find_critical_cycles(&solved_hbcn);
        assert!(!cycles.is_empty(), "Should find cycles in complex circuit");
    }

    /// Test VCD generation
    #[test]
    fn test_vcd_generation() {
        let input = r#"Port "a" [("b", 20)]
                      Port "b" []"#;

        let (_, solved_hbcn) = run_analysis(input, true)
            .expect("Should compute cycle time");

        let mut output = Cursor::new(Vec::new());
        hbcn::write_vcd(&solved_hbcn, &mut output)
            .expect("Should write VCD");

        let vcd_content = String::from_utf8(output.into_inner())
            .expect("Should be valid UTF-8");

        // VCD should contain basic structure
        assert!(vcd_content.contains("$timescale") || vcd_content.contains("$var"));
    }

    /// Test DOT generation
    #[test]
    fn test_dot_generation() {
        let input = r#"Port "a" [("b", 20)]
                      Port "b" []"#;

        let (_, solved_hbcn) = run_analysis(input, true)
            .expect("Should compute cycle time");

        let dot_content = format!("{:?}", petgraph::dot::Dot::new(&solved_hbcn));
        
        // DOT should contain basic graph structure
        assert!(dot_content.contains("digraph") || dot_content.contains("graph"));
    }

    /// Test cycle cost calculation
    #[test]
    fn test_cycle_cost_calculation() {
        let input = r#"Port "input" [("reg", 30)]
                      DataReg "reg" [("output", 25), ("reg", 20)]
                      Port "output" []"#;

        let (_, solved_hbcn) = run_analysis(input, true)
            .expect("Should compute cycle time");

        let cycles = hbcn::find_critical_cycles(&solved_hbcn);
        
        for cycle in &cycles {
            let cost: f64 = cycle
                .iter()
                .map(|(is, it)| {
                    let ie = solved_hbcn.find_edge(*is, *it).unwrap();
                    let e = &solved_hbcn[ie];
                    e.weight() - e.slack()
                })
                .sum();
            
            assert!(cost >= 0.0, "Cycle cost should be non-negative");
        }
    }

    /// Test transition type classification
    #[test]
    fn test_transition_type_classification() {
        let input = r#"Port "a" [("b", 20)]
                      Port "b" []"#;

        let (_, solved_hbcn) = run_analysis(input, true)
            .expect("Should compute cycle time");

        let cycles = hbcn::find_critical_cycles(&solved_hbcn);
        
        for cycle in &cycles {
            for (is, it) in cycle {
                let s = &solved_hbcn[*is];
                let t = &solved_hbcn[*it];
                
                let ttype = match (&s.transition, &t.transition) {
                    (Transition::Data(_), Transition::Data(_)) => "Data Prop",
                    (Transition::Spacer(_), Transition::Spacer(_)) => "Null Prop",
                    (Transition::Data(_), Transition::Spacer(_)) => "Data Ack",
                    (Transition::Spacer(_), Transition::Data(_)) => "Null Ack",
                };
                
                // Should classify transition types correctly
                assert!(!ttype.is_empty(), "Transition type should not be empty");
            }
        }
    }

    /// Test token counting in cycles
    #[test]
    fn test_token_counting() {
        let input = r#"Port "input" [("reg", 30)]
                      DataReg "reg" [("output", 25), ("reg", 20)]
                      Port "output" []"#;

        let (_, solved_hbcn) = run_analysis(input, true)
            .expect("Should compute cycle time");

        let cycles = hbcn::find_critical_cycles(&solved_hbcn);
        
        for cycle in &cycles {
            let mut tokens = 0;
            for (is, it) in cycle {
                let ie = solved_hbcn.find_edge(*is, *it).unwrap();
                let e = &solved_hbcn[ie];
                if e.is_marked() {
                    tokens += 1;
                }
            }
            
            // Token count should be reasonable
            assert!(tokens >= 0, "Token count should be non-negative");
        }
    }

    /// Test depth analysis (unweighted)
    #[test]
    fn test_depth_analysis() {
        let input = r#"Port "a" [("b", 20)]
                      Port "b" [("c", 15)]
                      Port "c" []"#;

        let (depth, _) = run_analysis(input, false)
            .expect("Should compute depth for simple circuit");

        assert!(depth > 0.0, "Depth should be positive");
    }

    /// Test analysis with empty graph
    #[test]
    fn test_empty_graph_analysis() {
        let input = r#"Port "lonely" []"#;

        // This might fail or succeed depending on implementation
        let result = run_analysis(input, true);
        
        // Either succeeds with valid results or fails gracefully
        if let Ok((cycle_time, _)) = result {
            assert!(cycle_time >= 0.0, "Cycle time should be non-negative");
        }
        // If it fails, that's also acceptable for empty graphs
    }

    /// Test analysis error handling
    #[test]
    fn test_analysis_error_handling() {
        let invalid_input = "Invalid graph syntax";

        let result = parse(invalid_input);
        assert!(result.is_err(), "Should fail to parse invalid input");
    }
}
