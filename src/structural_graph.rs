mod ast;

use lalrpop_util;
lalrpop_util::lalrpop_mod! {parser, "/structural_graph/parser.rs"}

use ast::{Entry, EntryType};
use petgraph::{graph, stable_graph::StableGraph};
use std::collections::HashMap;
use string_cache::DefaultAtom;

type Symbol = DefaultAtom;

/// Identifier of a register or port component
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitNode {
    Port(Symbol),
    Register(Symbol),
}

impl CircuitNode {
    pub fn name(&self) -> Symbol {
        match self {
            CircuitNode::Port(name) => name.clone(),
            CircuitNode::Register(name) => name.clone(),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel {
    pub initial_phase: ChannelPhase,
    pub is_internal: bool,
}

pub type StructuralGraph = StableGraph<CircuitNode, Channel>;

type LarlPopError<'a> = lalrpop_util::ParseError<usize, parser::Token<'a>, &'static str>;

/// Error Response of StructuralGraph::parse
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError<'a> {
    SyntaxError(LarlPopError<'a>),
    MultipleDefinitions(CircuitNode),
    UndefinedElement(Symbol),
}

impl<'a> From<LarlPopError<'a>> for ParseError<'a> {
    fn from(err: LarlPopError) -> ParseError {
        ParseError::SyntaxError(err)
    }
}

/// Parse StructuralGraph description generate by pulsar's syn_rtl
pub fn parse(input: &str) -> Result<StructuralGraph, ParseError> {
    let nodes = parser::GraphParser::new().parse(input)?;

    let mut ret = StructuralGraph::new();
    let mut lut = HashMap::new();

    let mut adjacency: Vec<(graph::NodeIndex, Channel, Vec<Symbol>)> = Vec::new();

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

                let cn = CircuitNode::Register(name.clone());
                let cni = ret.add_node(cn);
                lut.insert(name, cni);

                let s0n = CircuitNode::Register(s0.clone());
                let s0i = ret.add_node(s0n);
                lut.insert(s0, s0i);
                ret.add_edge(
                    cni,
                    s0i,
                    Channel {
                        initial_phase: ChannelPhase::ReqNull,
                        is_internal: true,
                    },
                );
                adjacency.push((
                    s0i,
                    Channel {
                        initial_phase: ChannelPhase::ReqData,
                        is_internal: true,
                    },
                    vec![s1.clone()],
                ));

                CircuitNode::Register(s1)
            }
            EntryType::Port => CircuitNode::Port(name),
            EntryType::NullReg => CircuitNode::Register(name),
        };
        let ni = ret.add_node(c.clone());
        if let Some(_) = lut.insert(c.name(), ni) {
            return Err(ParseError::MultipleDefinitions(c));
        }
        adjacency.push((
            ni,
            Channel {
                initial_phase: ChannelPhase::AckNull,
                is_internal: false,
            },
            adjacency_list,
        ));
    }

    for (ni, channel, adjacency_list) in adjacency.into_iter() {
        for x in adjacency_list.into_iter() {
            if let Some(xi) = lut.get(&x) {
                ret.add_edge(ni, *xi, channel.clone());
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
