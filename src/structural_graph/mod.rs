mod ast;

lalrpop_util::lalrpop_mod! {parser, "/structural_graph/parser.rs"}

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

/// Channel phase to be used when expanding from StructuralGraph to HBCN
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
            Port "a" ["result"]
            Port "b" ["result"]
            NullReg "result" ["acc", "output"]
            DataReg "acc" ["result"]
            Port "output" []
            "#;
        let result = parse(input);
        assert!(matches!(result, Ok(_)));

        let g = result.unwrap();
        assert_eq!(g.edge_count(), 7);
        assert_eq!(g.node_count(), 7);
    }

    #[test]
    fn parse_err_undefined() {
        let input = r#"
            Port "a" ["result"]
            Port "b" ["result"]
            NullReg "result" ["acc", "output"]
            DataReg "acc" ["result"]
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
            Port "a" ["result"]
            Port "b" ["result"]
            NullReg "result" ["acc" "output"]
            DataReg "acc" ["result"]
            Port "output" []
            "#;
        let result = parse(input);
        assert!(matches!(result, Err(ParseError::SyntaxError(_))));
    }
}
