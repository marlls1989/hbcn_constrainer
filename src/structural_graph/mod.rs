mod ast;

// Include the generated parser with clippy warnings suppressed
#[allow(clippy::all)]
mod parser {
    #![allow(clippy::all)]
    #![allow(dead_code)]
    #![allow(unused_variables)]
    #![allow(unused_imports)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    #![allow(non_upper_case_globals)]
    include!(concat!(env!("OUT_DIR"), "/structural_graph/parser.rs"));
}

use ast::{Entry, EntryType};
use petgraph::{graph, stable_graph::StableGraph};
use std::{collections::HashMap, error::Error, fmt};
use string_cache::DefaultAtom;

pub type Symbol = DefaultAtom;

// Cost constants for different register types
const REGISTER_COST: usize = 10;
const CONTROL_REG_COST: usize = 50;

/// Identifier of a register or port component
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CircuitNode {
    Port(Symbol),
    Register { name: Symbol, cost: usize },
}

impl CircuitNode {
    pub fn name(&self) -> &Symbol {
        match self {
            CircuitNode::Port(name) => name,
            CircuitNode::Register { name, .. } => name,
        }
    }

    pub fn base_cost(&self) -> usize {
        match self {
            CircuitNode::Port(_) => 0,
            CircuitNode::Register { cost, .. } => *cost,
        }
    }
}

impl fmt::Display for CircuitNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitNode::Port(name) => write!(f, "Port \"{}\"", name),
            CircuitNode::Register { name, cost } => {
                write!(f, "Register \"{}\" with cost {}", name, cost)
            }
        }
    }
}

/// Channel phase to be used when expanding from StructuralGraph to StructuralHBCN
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelPhase {
    AckNull,
    ReqData,
    AckData,
    ReqNull,
}

/// Channel representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Channel {
    pub initial_phase: ChannelPhase,
    pub is_internal: bool,
    pub virtual_delay: f64,
}

pub type StructuralGraph = StableGraph<CircuitNode, Channel>;

type LarlPopError<'a> = lalrpop_util::ParseError<usize, parser::Token<'a>, &'static str>;

/// Error Response of StructuralGraph::parse
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    SyntaxError(String),
    MultipleDefinitions(CircuitNode),
    UndefinedElement(Symbol),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::SyntaxError(err) => write!(f, "{}", err),
            ParseError::MultipleDefinitions(node) => {
                write!(f, "Multiple Definitions of {}", node.name())
            }
            ParseError::UndefinedElement(name) => write!(f, "Undefined Element: {}", name),
        }
    }
}

impl Error for ParseError {}

impl From<LarlPopError<'_>> for ParseError {
    fn from(err: LarlPopError) -> Self {
        ParseError::SyntaxError(format!("{}", err))
    }
}

/// Parse StructuralGraph description generate by pulsar's syn_rtl
pub fn parse(input: &str) -> Result<StructuralGraph, ParseError> {
    let nodes = parser::GraphParser::new().parse(input)?;

    let mut ret = StructuralGraph::new();
    let mut lut = HashMap::new();

    let mut adjacency: Vec<(graph::NodeIndex, Vec<(Symbol, Channel)>)> = Vec::new();

    for Entry {
        entry_type,
        name,
        adjacency_list,
    } in nodes.into_iter()
    {
        let c = match entry_type {
            EntryType::DataReg => {
                let s0: Symbol = format!("{}/s0", name.as_ref()).into();
                let s1: Symbol = format!("{}/s1", name.as_ref()).into();

                let cn = CircuitNode::Register {
                    name: name.clone(),
                    cost: REGISTER_COST,
                };
                let cni = ret.add_node(cn);
                lut.insert(name, cni);

                let s0n = CircuitNode::Register {
                    name: s0.clone(),
                    cost: REGISTER_COST,
                };
                let s0i = ret.add_node(s0n);
                lut.insert(s0.clone(), s0i);
                adjacency.push((
                    cni,
                    vec![(
                        s0,
                        Channel {
                            initial_phase: ChannelPhase::ReqNull,
                            is_internal: true,
                            virtual_delay: 10.0,
                        },
                    )],
                ));
                adjacency.push((
                    s0i,
                    vec![(
                        s1.clone(),
                        Channel {
                            initial_phase: ChannelPhase::ReqData,
                            is_internal: true,
                            virtual_delay: 10.0,
                        },
                    )],
                ));

                CircuitNode::Register {
                    name: s1,
                    cost: REGISTER_COST,
                }
            }
            EntryType::UnsafeReg => {
                let s0: Symbol = format!("{}/s0", name.as_ref()).into();

                let cn = CircuitNode::Register {
                    name: name.clone(),
                    cost: REGISTER_COST,
                };
                let cni = ret.add_node(cn);
                lut.insert(name, cni);

                adjacency.push((
                    cni,
                    vec![(
                        s0.clone(),
                        Channel {
                            initial_phase: ChannelPhase::ReqNull,
                            is_internal: true,
                            virtual_delay: 10.0,
                        },
                    )],
                ));

                CircuitNode::Register {
                    name: s0,
                    cost: REGISTER_COST,
                }
            }
            EntryType::Port => CircuitNode::Port(name),
            EntryType::NullReg => CircuitNode::Register {
                name,
                cost: REGISTER_COST,
            },
            EntryType::ControlReg => CircuitNode::Register {
                name,
                cost: CONTROL_REG_COST,
            },
        };
        let ni = ret.add_node(c.clone());
        if lut.insert(c.name().clone(), ni).is_some() {
            return Err(ParseError::MultipleDefinitions(c));
        }
        adjacency.push((
            ni,
            adjacency_list
                .into_iter()
                .map(|(s, n)| {
                    (
                        s,
                        if entry_type == EntryType::UnsafeReg {
                            Channel {
                                initial_phase: ChannelPhase::ReqData,
                                is_internal: true,
                                virtual_delay: n,
                            }
                        } else {
                            Channel {
                                initial_phase: ChannelPhase::AckNull,
                                is_internal: false,
                                virtual_delay: n,
                            }
                        },
                    )
                })
                .collect(),
        ));
    }

    for (ni, adjacency_list) in adjacency.into_iter() {
        for (x, channel) in adjacency_list.into_iter() {
            if let Some(xi) = lut.get(&x) {
                ret.add_edge(ni, *xi, channel);
            } else {
                return Err(ParseError::UndefinedElement(x.clone()));
            }
        }
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid() {
        let input = r#"
            Port "a" [("result", 10)]
            Port "b" [("result", 20)]
            NullReg "result" [("acc", 30), ("output", 40)]
            DataReg "acc" [("result", 50)]
            Port "output" []
            "#;
        let result = parse(input);
        assert!(result.is_ok());

        let g = result.unwrap();
        assert_eq!(g.edge_count(), 7);
        assert_eq!(g.node_count(), 7);
    }

    #[test]
    fn parse_err_undefined() {
        let input = r#"
            Port "a" [("result", 10)]
            Port "b" [("result", 20)]
            NullReg "result" [("acc", 30), ("output", 40)]
            DataReg "acc" [("result", 50)]
            "#;
        let result = parse(input);
        assert!(matches!(result, Err(ParseError::UndefinedElement(_))));
        if let Err(ParseError::UndefinedElement(node)) = result {
            assert_eq!(node.as_ref(), "output");
        }
    }

    #[test]
    fn parse_err_syntax() {
        let input = r#"
            Port "a" [("result", 10)]
            Port "b" [("result", 20)]
            NullReg "result" [("acc", 30) ("output", 40)]
            DataReg "acc" [("result", 50)]
            Port "output" []
            "#;
        let result = parse(input);
        assert!(matches!(result, Err(ParseError::SyntaxError(_))));
    }

    #[test]
    fn parse_realistic_port_names() {
        let input = r#"
            Port "port:ARV/instruction[31]" [("inst:ARV/decode_retime_s1_94_reg", 130), ("inst:ARV/decode_retime_s1_137_reg", 100)]
            Port "port:ARV/mem_data_in[0]" []
            DataReg "inst:ARV/decode_retime_s1_94_reg" [("inst:ARV/some_output_reg", 75)]
            DataReg "inst:ARV/decode_retime_s1_137_reg" [("inst:ARV/some_output_reg", 85)]
            DataReg "inst:ARV/some_output_reg" []
            "#;
        let result = parse(input);
        assert!(result.is_ok());

        let g = result.unwrap();
        // 2 Ports (2 nodes) + 3 DataRegs (9 nodes, 3 each) = 11 nodes total
        assert_eq!(g.node_count(), 11);
        // Two ports connecting to two DataRegs, which both connect to one output DataReg
        // Plus internal connections for DataRegs: 2 + 2 + 2 + 2 + 2 = 10 edges
        assert_eq!(g.edge_count(), 10);
    }

    #[test]
    fn parse_complex_adjacency_list() {
        let input = r#"
            Port "input" [("reg1", 10), ("reg2", 20), ("reg3", 30), ("reg4", 40)]
            DataReg "reg1" [("output", 50)]
            DataReg "reg2" [("output", 60)]
            DataReg "reg3" [("output", 70)]
            DataReg "reg4" [("output", 80)]
            Port "output" []
            "#;
        let result = parse(input);
        assert!(result.is_ok());

        let g = result.unwrap();
        // 2 Ports (2 nodes) + 4 DataRegs (12 nodes, 3 each) = 14 nodes total
        assert_eq!(g.node_count(), 14);
        // 4 connections from input to DataRegs + 4 connections from DataRegs to output + 8 internal DataReg connections (2 each) = 16 edges
        assert_eq!(g.edge_count(), 16);
    }

    #[test]
    fn parse_all_register_types() {
        let input = r#"
            Port "input" [("data_reg", 100)]
            DataReg "data_reg" [("null_reg", 200)]
            NullReg "null_reg" [("control_reg", 300)]
            ControlReg "control_reg" [("unsafe_reg", 400)]
            UnsafeReg "unsafe_reg" [("output", 500)]
            Port "output" []
            "#;
        let result = parse(input);
        assert!(result.is_ok());

        let g = result.unwrap();
        // 2 Ports + 1 DataReg (3 nodes) + 1 NullReg + 1 ControlReg + 1 UnsafeReg (2 nodes) = 9 nodes total
        assert_eq!(g.node_count(), 9);
        // 5 explicit connections + 2 internal DataReg connections + 1 internal UnsafeReg connection = 8 edges
        assert_eq!(g.edge_count(), 8);
    }

    #[test]
    fn parse_floating_point_weights() {
        let input = r#"
            Port "input" [("reg1", 10.5), ("reg2", 20.75)]
            DataReg "reg1" [("output", 100.0)]
            DataReg "reg2" [("output", 150.25)]
            Port "output" []
            "#;
        let result = parse(input);
        assert!(result.is_ok());

        let g = result.unwrap();
        // 2 Ports + 2 DataRegs (6 nodes, 3 each) = 8 nodes total
        assert_eq!(g.node_count(), 8);
        // 2 connections from input to DataRegs + 2 connections from DataRegs to output + 4 internal DataReg connections = 8 edges
        assert_eq!(g.edge_count(), 8);
    }

    #[test]
    fn parse_test_graph_format() {
        // Test the exact format from test.graph file
        let input = r#"Port "a" [("b", 20)]
Port "b" []"#;
        let result = parse(input);
        assert!(result.is_ok());

        let g = result.unwrap();
        assert_eq!(g.node_count(), 2); // Two ports
        assert_eq!(g.edge_count(), 1); // One connection from a to b
    }
}
