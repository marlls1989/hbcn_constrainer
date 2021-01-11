mod ast;

use lalrpop_util;
lalrpop_util::lalrpop_mod! {parser, "/structural_graph/parser.rs"}

use ast::{Entry, EntryType};
use coin_cbc::{Col, Model, Sense};
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

pub fn slack_match(
    g: &StructuralGraph,
    buffer_delay: f64,
    cycle_time: f64,
) -> Option<StableGraph<(CircuitNode, f64, f64), (Channel, (u32, f64, f64))>> {
    let stage_delay = cycle_time / 4.;
    let mut m = Model::default();

    let arr_pairs: HashMap<graph::NodeIndex, (Col, Col)> = g
        .node_indices()
        .map(|ix| (ix, (m.add_col(), m.add_col())))
        .collect();

    let slack_buffers: HashMap<graph::EdgeIndex, (Col, Col, Col)> = g
        .edge_indices()
        .filter_map(|ie| {
            let (ref src, ref dst) = g.edge_endpoints(ie)?;
            let (src_data, src_null) = arr_pairs.get(src)?;
            let (dst_data, dst_null) = arr_pairs.get(dst)?;
            let e = g[ie];
            let stage_delay = if e.is_internal {
                buffer_delay
            } else {
                stage_delay
            };

            let fwd_data = m.add_row();
            m.set_row_lower(
                fwd_data,
                stage_delay
                    - if e.initial_phase == ChannelPhase::ReqData {
                        cycle_time
                    } else {
                        0.
                    },
            );
            m.set_weight(fwd_data, *dst_data, 1.);
            m.set_weight(fwd_data, *src_data, -1.);

            let fwd_null = m.add_row();
            m.set_row_lower(
                fwd_null,
                stage_delay
                    - if e.initial_phase == ChannelPhase::ReqNull {
                        cycle_time
                    } else {
                        0.
                    },
            );
            m.set_weight(fwd_null, *dst_null, 1.);
            m.set_weight(fwd_null, *src_null, -1.);

            let bwd_data = m.add_row();
            m.set_row_lower(
                bwd_data,
                stage_delay
                    - if e.initial_phase == ChannelPhase::AckData {
                        cycle_time
                    } else {
                        0.
                    },
            );
            m.set_weight(bwd_data, *src_null, 1.);
            m.set_weight(bwd_data, *dst_data, -1.);

            let bwd_null = m.add_row();
            m.set_row_lower(
                bwd_null,
                stage_delay
                    - if e.initial_phase == ChannelPhase::AckNull {
                        cycle_time
                    } else {
                        0.
                    },
            );
            m.set_weight(bwd_null, *src_data, 1.);
            m.set_weight(bwd_null, *dst_null, -1.);

            if e.is_internal {
                None
            } else {
                let fwd_setup = m.add_col();
                m.set_col_upper(fwd_setup, stage_delay / 4.);
                let bwd_setup = m.add_col();
                m.set_col_upper(bwd_setup, stage_delay / 4.);

                m.set_weight(fwd_data, fwd_setup, 1.);
                m.set_weight(fwd_null, fwd_setup, 1.);
                m.set_weight(bwd_data, bwd_setup, 1.);
                m.set_weight(bwd_null, bwd_setup, 1.);

                m.set_obj_coeff(fwd_setup, 10.);
                m.set_obj_coeff(bwd_setup, 10.);

                let buf_count = m.add_integer();
                let delta = match e.initial_phase {
                    ChannelPhase::AckNull | ChannelPhase::AckData => stage_delay,
                    ChannelPhase::ReqNull | ChannelPhase::ReqData => -stage_delay,
                };
                m.set_weight(fwd_data, buf_count, -delta);
                m.set_weight(fwd_null, buf_count, -delta);
                m.set_weight(bwd_data, buf_count, delta);
                m.set_weight(bwd_null, buf_count, delta);

                m.set_obj_coeff(buf_count, 1.);
                Some((ie, (buf_count, fwd_setup, bwd_setup)))
            }
        })
        .collect();

    m.set_obj_sense(Sense::Minimize);
    let sol = m.solve();

    if sol.raw().is_proven_infeasible() || sol.raw().is_initial_solve_proven_primal_infeasible() {
        None
    } else {
        Some(g.map(
            |ix, x| {
                let (d, n) = arr_pairs.get(&ix).unwrap();
                (x.clone(), sol.col(*d), sol.col(*n))
            },
            |ie, e| {
                (
                    e.clone(),
                    slack_buffers.get(&ie).map_or((0, 0., 0.), |(x, y, z)| {
                        (sol.col(*x).round() as u32, sol.col(*y), sol.col(*z))
                    }),
                )
            },
        ))
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
