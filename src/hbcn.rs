use super::structural_graph::{ChannelPhase, CircuitNode, StructuralGraph};
use petgraph::{graph::NodeIndex, stable_graph::StableGraph};

use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug)]
pub enum Transition {
    Spacer(CircuitNode),
    Data(CircuitNode),
}

#[derive(PartialEq, Eq, Debug)]
pub enum Place {
    ForwardPropagation {
        token: bool,
    },
    BackwardPropagation {
        token: bool,
    },
    ReflexivePropagation {
        token: bool,
        related: Vec<Transition>,
    },
}

pub type HBCN = StableGraph<Transition, Place>;

pub fn from_structural_graph(g: &StructuralGraph) -> Option<HBCN> {
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
        let initial_phase = g[ix].initial_phase;

        ret.add_edge(
            *src_token,
            *dst_token,
            Place::ForwardPropagation {
                token: initial_phase == ChannelPhase::ReqData,
            },
        );
        ret.add_edge(
            *src_spacer,
            *dst_spacer,
            Place::ForwardPropagation {
                token: initial_phase == ChannelPhase::ReqNull,
            },
        );
        ret.add_edge(
            *dst_token,
            *src_spacer,
            Place::BackwardPropagation {
                token: initial_phase == ChannelPhase::AckData,
            },
        );
        ret.add_edge(
            *dst_spacer,
            *src_token,
            Place::BackwardPropagation {
                token: initial_phase == ChannelPhase::AckNull,
            },
        );
    }

    Some(ret)
}
