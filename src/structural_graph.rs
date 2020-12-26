mod ast;

use lalrpop_util;
lalrpop_util::lalrpop_mod! {parser, "/structural_graph/parser.rs"}

use ast::{Entry, EntryType};
use petgraph::{graph, Graph};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum CircuitNode {
    Port { name: Rc<str> },
    Register { name: Rc<str> },
}

#[derive(Debug, Clone, Copy)]
pub enum ChannelPhase {
    AckNull,
    ReqData,
    AckData,
    ReqNull,
}

#[derive(Debug, Clone)]
pub struct Channel {
    pub initial_phase: ChannelPhase,
    pub is_internal: bool,
}

impl CircuitNode {
    pub fn name(&self) -> Rc<str> {
        match self {
            CircuitNode::Port { name } => name.clone(),
            CircuitNode::Register { name } => name.clone(),
        }
    }
}

type InnerGraphT = petgraph::Graph<CircuitNode, Channel>;

#[derive(Debug)]
pub struct StructuralGraph {
    pub inner: InnerGraphT,
}

type LarlPopError<'a> = lalrpop_util::ParseError<usize, parser::Token<'a>, &'static str>;

#[derive(Debug)]
pub enum ParseError<'a> {
    SyntaxError(LarlPopError<'a>),
    MultipleDefinitions(CircuitNode),
    UndefinedElement(Rc<str>),
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
    pub fn parse(input: &str) -> Result<StructuralGraph, ParseError> {
        let nodes = parser::GraphParser::new().parse(input)?;

        let mut ret = StructuralGraph {
            inner: Graph::new(),
        };

        let mut lut: HashMap<Rc<str>, graph::NodeIndex> = HashMap::new();
        let mut adjacency: Vec<(graph::NodeIndex, Channel, Vec<Rc<str>>)> = Vec::new();

        for Entry {
            entry_type,
            name,
            adjacency_list,
        } in nodes.into_iter()
        {
            let c = match entry_type {
                EntryType::DataReg => {
                    let s0: Rc<str> = format!("{}/s0", name).into();
                    let s1: Rc<str> = format!("{}/s1", name).into();

                    let cn = CircuitNode::Register { name: name.clone() };
                    let cni = ret.inner.add_node(cn);
                    lut.insert(name, cni);

                    let s0n = CircuitNode::Register { name: s0.clone() };
                    let s0i = ret.inner.add_node(s0n);
                    lut.insert(s0, s0i);
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
                if let Some(xi) = lut.get(x.as_ref()) {
                    ret.inner.add_edge(ni, *xi, channel.clone());
                } else {
                    return Err(ParseError::UndefinedElement(x.clone()));
                }
            }
        }

        Ok(ret)
    }
}
