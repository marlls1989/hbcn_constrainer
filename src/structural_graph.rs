mod ast;

use lalrpop_util;
lalrpop_util::lalrpop_mod! {parser, "/structural_graph/parser.rs"}

use ast::{Entry, EntryType};
use immutable_string::ImmutableString;
use petgraph::{graph, stable_graph::StableGraph};
use std::collections::HashMap;

type Symbol = ImmutableString;

/// Identifier of a register or port component
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitNode {
    Port { name: Symbol },
    Register { name: Symbol },
}

impl CircuitNode {
    pub fn name(&self) -> Symbol {
        match self {
            CircuitNode::Port { name } => name.clone(),
            CircuitNode::Register { name } => name.clone(),
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

type InnerGraphT = StableGraph<CircuitNode, Channel>;

#[derive(Debug)]
pub struct StructuralGraph {
    inner: InnerGraphT,
    lut: HashMap<Symbol, graph::NodeIndex>,
}

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

impl From<StructuralGraph> for InnerGraphT {
    fn from(graph: StructuralGraph) -> InnerGraphT {
        graph.inner
    }
}

impl StructuralGraph {
    /// Get reference to internal petgraph representantion
    pub fn inner_ref(&self) -> &InnerGraphT {
        &self.inner
    }

    /// Parse StructuralGraph description generate by pulsar's syn_rtl
    pub fn parse(input: &str) -> Result<StructuralGraph, ParseError> {
        let nodes = parser::GraphParser::new().parse(input)?;

        let mut ret = StructuralGraph {
            inner: InnerGraphT::new(),
            lut: HashMap::new(),
        };

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

                    let cn = CircuitNode::Register { name: name.clone() };
                    let cni = ret.inner.add_node(cn);
                    ret.lut.insert(name, cni);

                    let s0n = CircuitNode::Register { name: s0.clone() };
                    let s0i = ret.inner.add_node(s0n);
                    ret.lut.insert(s0, s0i);
                    ret.inner.add_edge(
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

                    CircuitNode::Register { name: s1 }
                }
                EntryType::Port => CircuitNode::Port { name },
                EntryType::NullReg => CircuitNode::Register { name },
            };
            let ni = ret.inner.add_node(c.clone());
            if let Some(_) = ret.lut.insert(c.name(), ni) {
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
                if let Some(xi) = ret.lut.get(x.as_ref()) {
                    ret.inner.add_edge(ni, *xi, channel.clone());
                } else {
                    return Err(ParseError::UndefinedElement(x.clone()));
                }
            }
        }

        Ok(ret)
    }
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
        let result = StructuralGraph::parse(input);
        assert!(matches!(result, Ok(_)));

        let result = result.unwrap();
        let g = result.inner_ref();
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
        let result = StructuralGraph::parse(input);
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
        let result = StructuralGraph::parse(input);
        assert!(matches!(result, Err(ParseError::SyntaxError(_))));
    }
}
