use super::structural_graph::{Channel, ChannelPhase, CircuitNode, StructuralGraph};
use coin_cbc::{Col, Model, Row};
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    stable_graph::StableGraph,
    EdgeDirection,
};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Transition {
    Spacer(CircuitNode),
    Data(CircuitNode),
}

#[derive(PartialEq, Debug, Clone)]
pub struct Place {
    token: bool,
    max_delay: Option<f64>,
    min_delay: Option<f64>,
    relative_endpoints: Vec<NodeIndex>,
}

pub type HBCN = StableGraph<Transition, Place>;

pub fn from_structural_graph(g: &StructuralGraph, internal_delay: f64) -> Option<HBCN> {
    let mut ret = HBCN::new();
    let vertice_map: HashMap<NodeIndex, (NodeIndex, NodeIndex)> = g
        .node_indices()
        .map(|ix| {
            let ref val = g[ix];
            let token = ret.add_node(Transition::Data(val.clone()));
            let spacer = ret.add_node(Transition::Spacer(val.clone()));
            (ix, (token, spacer))
        })
        .collect();

    for ix in g.edge_indices() {
        let (ref src, ref dst) = g.edge_endpoints(ix)?;
        let (src_token, src_spacer) = vertice_map.get(src)?;
        let (dst_token, dst_spacer) = vertice_map.get(dst)?;
        let Channel {
            initial_phase,
            is_internal,
        } = g[ix];

        ret.add_edge(
            *src_token,
            *dst_token,
            Place {
                token: initial_phase == ChannelPhase::ReqData,
                max_delay: if is_internal {
                    Some(internal_delay)
                } else {
                    None
                },
                min_delay: None,
                relative_endpoints: Vec::new(),
            },
        );
        ret.add_edge(
            *src_spacer,
            *dst_spacer,
            Place {
                token: initial_phase == ChannelPhase::ReqNull,
                max_delay: if is_internal {
                    Some(internal_delay)
                } else {
                    None
                },
                min_delay: None,
                relative_endpoints: Vec::new(),
            },
        );
        ret.add_edge(
            *dst_token,
            *src_spacer,
            Place {
                token: initial_phase == ChannelPhase::AckData,
                max_delay: if is_internal {
                    Some(internal_delay)
                } else {
                    None
                },
                min_delay: None,
                relative_endpoints: Vec::new(),
            },
        );
        ret.add_edge(
            *dst_spacer,
            *src_token,
            Place {
                token: initial_phase == ChannelPhase::AckNull,
                max_delay: if is_internal {
                    Some(internal_delay)
                } else {
                    None
                },
                min_delay: None,
                relative_endpoints: Vec::new(),
            },
        );
    }

    for ix in g.node_indices() {
        let (ix_data, ix_null) = vertice_map.get(&ix)?;
        for is in g.neighbors_directed(ix, EdgeDirection::Incoming) {
            let (is_data, is_null) = vertice_map.get(&is)?;
            for id in g.neighbors_directed(ix, EdgeDirection::Incoming) {
                let (id_data, id_null) = vertice_map.get(&id)?;
                if let Some(ie) = ret.find_edge(*is_data, *id_null) {
                    ret[ie].relative_endpoints.push(*ix_data);
                } else {
                    ret.add_edge(
                        *is_data,
                        *id_null,
                        Place {
                            token: ret[ret.find_edge(*is_data, *ix_data)?].token
                                || ret[ret.find_edge(*ix_data, *id_null)?].token,
                            min_delay: None,
                            max_delay: None,
                            relative_endpoints: vec![*ix_data],
                        },
                    );
                }
                if let Some(ie) = ret.find_edge(*is_null, *id_data) {
                    ret[ie].relative_endpoints.push(*ix_null);
                } else {
                    ret.add_edge(
                        *is_null,
                        *id_data,
                        Place {
                            token: ret[ret.find_edge(*is_null, *ix_null)?].token
                                || ret[ret.find_edge(*ix_null, *id_data)?].token,
                            min_delay: None,
                            max_delay: None,
                            relative_endpoints: vec![*ix_null],
                        },
                    );
                }
            }
        }
    }

    Some(ret)
}

pub fn constraint_cycle_time(hbcn: &HBCN) -> Option<HBCN> {
    let mut m = Model::default();
    let pseudo_clock = m.add_col();
    let arr_var: HashMap<NodeIndex, Col> = hbcn.node_indices().map(|x| (x, m.add_col())).collect();
    let delay_var: HashMap<EdgeIndex, Col> =
        hbcn.edge_indices().map(|x| (x, m.add_col())).collect();

    None
}
