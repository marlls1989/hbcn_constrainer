use super::{
    structural_graph::{Channel, ChannelPhase, CircuitNode, StructuralGraph},
    SolverError,
};
use coin_cbc::{Col, Model, Sense};
use gag::Gag;
use petgraph::{graph, stable_graph::StableGraph};
use std::collections::HashMap;

pub fn slack_match(
    g: &StructuralGraph,
    internal_delay: f64,
    cycle_time: f64,
) -> Result<StableGraph<(CircuitNode, f64, f64, bool), (Channel, u32)>, SolverError> {
    let _redirect_stdout = Gag::stdout();
    let _redirect_stderr = Gag::stderr();
    let stage_delay = cycle_time / 4.;
    let mut m = Model::default();

    let arr_pairs: HashMap<graph::NodeIndex, (Col, Col, Option<Col>)> = g
        .node_indices()
        .map(|ix| {
            (
                ix,
                (
                    m.add_col(),
                    m.add_col(),
                    match g[ix] {
                        CircuitNode::Port(_)
                        | CircuitNode::Register {
                            protected: true, ..
                        } => None,
                        CircuitNode::Register {
                            protected: false, ..
                        } => Some(m.add_col()),
                    },
                ),
            )
        })
        .collect();

    let slack_buffers: HashMap<graph::EdgeIndex, Col> = g
        .edge_indices()
        .filter_map(|ie| {
            let (ref src, ref dst) = g.edge_endpoints(ie)?;
            let (src_data, src_null, _) = arr_pairs.get(src)?;
            let (dst_data, dst_null, dst_suppres) = arr_pairs.get(dst)?;
            let e = g[ie];
            let stage_delay = if e.is_internal {
                internal_delay
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

            if let Some(suppres) = dst_suppres {
                let delta = match e.initial_phase {
                    ChannelPhase::AckNull | ChannelPhase::AckData => stage_delay,
                    ChannelPhase::ReqNull | ChannelPhase::ReqData => -stage_delay,
                };

                m.set_weight(fwd_data, *suppres, delta);
                m.set_weight(fwd_null, *suppres, delta);
                m.set_weight(bwd_data, *suppres, -delta);
                m.set_weight(bwd_null, *suppres, -delta);
                m.set_obj_coeff(*suppres, 1.);
            }

            if e.is_internal {
                None
            } else {
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

                Some((ie, buf_count))
            }
        })
        .collect();

    m.set_obj_sense(Sense::Minimize);
    let sol = m.solve();

    if sol.raw().is_proven_infeasible() || sol.raw().is_initial_solve_proven_primal_infeasible() {
        Err(SolverError::Infeasible)
    } else {
        Ok(g.map(
            |ix, x| {
                let (d, n, suppres_stage) = arr_pairs.get(&ix).unwrap();
                (
                    x.clone(),
                    sol.col(*d),
                    sol.col(*n),
                    suppres_stage.map_or(false, |x| sol.col(x).round() as u32 != 0),
                )
            },
            |ie, e| {
                (
                    e.clone(),
                    slack_buffers
                        .get(&ie)
                        .map_or(0, |x| sol.col(*x).round() as u32),
                )
            },
        ))
    }
}
