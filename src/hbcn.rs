use super::{
    structural_graph::{Channel, ChannelPhase, CircuitNode, StructuralGraph, Symbol},
    SolverError,
};
use gurobi::{attr, ConstrSense::*, Env, Model, ModelSense::*, Status, Var, VarType::*, INFINITY};
use itertools::Itertools;
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    stable_graph::StableGraph,
    EdgeDirection,
};
use regex::Regex;
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    error::Error,
    fmt, io,
};
use vcd;

fn clog2(x: usize) -> u32 {
    const NUM_BITS: u32 = (std::mem::size_of::<usize>() as u32) * 8;
    NUM_BITS - x.leading_zeros()
}

#[derive(PartialEq, Eq, Debug, Clone, PartialOrd, Ord)]
pub enum Transition {
    Spacer(CircuitNode),
    Data(CircuitNode),
}

impl Transition {
    pub fn circuit_node(&self) -> &CircuitNode {
        match self {
            Transition::Data(id) => id,
            Transition::Spacer(id) => id,
        }
    }

    pub fn name(&self) -> &Symbol {
        self.circuit_node().name()
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transition::Spacer(id) => write!(f, "Spacer at {}", id),
            Transition::Data(id) => write!(f, "Data at {}", id),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Place {
    pub token: bool,
    pub weight: u32,
    pub relative_endpoints: Vec<NodeIndex>,
}

pub type HBCN = StableGraph<Transition, Place>;

pub fn from_structural_graph(g: &StructuralGraph, reflexive: bool) -> Option<HBCN> {
    let mut ret = HBCN::new();
    let vertice_map: HashMap<NodeIndex, (NodeIndex, NodeIndex, u32)> = g
        .node_indices()
        .map(|ix| {
            let ref val = g[ix];
            let token = ret.add_node(Transition::Data(val.clone()));
            let spacer = ret.add_node(Transition::Spacer(val.clone()));
            let backward_cost =
                25 + 10 * clog2(g.edges_directed(ix, petgraph::Direction::Outgoing).count());
            (ix, (token, spacer, backward_cost))
        })
        .collect();

    for ix in g.edge_indices() {
        let (ref src, ref dst) = g.edge_endpoints(ix)?;
        let (src_token, src_spacer, backward_cost) = vertice_map.get(src)?;
        let (dst_token, dst_spacer, _) = vertice_map.get(dst)?;
        let Channel {
            initial_phase,
            forward_cost,
            ..
        } = g[ix];

        ret.add_edge(
            *src_token,
            *dst_token,
            Place {
                token: initial_phase == ChannelPhase::ReqData,
                relative_endpoints: Vec::new(),
                weight: forward_cost,
            },
        );
        ret.add_edge(
            *src_spacer,
            *dst_spacer,
            Place {
                token: initial_phase == ChannelPhase::ReqNull,
                relative_endpoints: Vec::new(),
                weight: forward_cost,
            },
        );
        ret.add_edge(
            *dst_token,
            *src_spacer,
            Place {
                token: initial_phase == ChannelPhase::AckData,
                relative_endpoints: Vec::new(),
                weight: *backward_cost,
            },
        );
        ret.add_edge(
            *dst_spacer,
            *src_token,
            Place {
                token: initial_phase == ChannelPhase::AckNull,
                relative_endpoints: Vec::new(),
                weight: *backward_cost,
            },
        );
    }

    if reflexive {
        // For all nodes ix in g
        for ix in g.node_indices() {
            let (ix_data, ix_null, backward_cost) = vertice_map.get(&ix)?;

            // Find all predecessors is of ix
            for is in g.neighbors_directed(ix, EdgeDirection::Incoming) {
                // get pair of transitions related to is
                let (is_data, is_null, _) = vertice_map.get(&is)?;

                // find the forward path places related to the transitions of is
                let Place {
                    weight: data_forward_cost,
                    ..
                } = ret[ret.find_edge(*is_data, *ix_data)?];
                let Place {
                    weight: null_forward_cost,
                    ..
                } = ret[ret.find_edge(*is_null, *ix_null)?];

                // Find all predecessors id of ix
                for id in g.neighbors_directed(ix, EdgeDirection::Incoming) {
                    let (id_data, id_null, _) = vertice_map.get(&id)?;

                    // If a path is established between is and id, update Place
                    // Else create a reflexive path between is and id
                    if let Some(ie) = ret.find_edge(*is_data, *id_null) {
                        ret[ie].relative_endpoints.push(*ix_data);
                        ret[ie].weight =
                            std::cmp::max(ret[ie].weight, backward_cost + data_forward_cost);
                    } else {
                        ret.add_edge(
                            *is_data,
                            *id_null,
                            Place {
                                token: ret[ret.find_edge(*is_data, *ix_data)?].token
                                    || ret[ret.find_edge(*ix_data, *id_null)?].token,
                                relative_endpoints: vec![*ix_data],
                                weight: backward_cost + data_forward_cost,
                            },
                        );
                    }
                    if let Some(ie) = ret.find_edge(*is_null, *id_data) {
                        ret[ie].relative_endpoints.push(*ix_null);
                        ret[ie].weight =
                            std::cmp::max(ret[ie].weight, backward_cost + null_forward_cost);
                    } else {
                        ret.add_edge(
                            *is_null,
                            *id_data,
                            Place {
                                token: ret[ret.find_edge(*is_null, *ix_null)?].token
                                    || ret[ret.find_edge(*ix_null, *id_data)?].token,
                                relative_endpoints: vec![*ix_null],
                                weight: backward_cost + null_forward_cost,
                            },
                        );
                    }
                }
            }
        }
    }

    Some(ret)
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TransitionEvent {
    pub time: u64,
    pub transition: Transition,
}

impl PartialOrd for TransitionEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TransitionEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SlackedPlace {
    pub slack: u64,
    pub place: Place,
}

impl PartialOrd for SlackedPlace {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SlackedPlace {
    fn cmp(&self, other: &Self) -> Ordering {
        self.slack.cmp(&other.slack)
    }
}

pub type SolvedHBCN = StableGraph<TransitionEvent, SlackedPlace>;

pub type PathConstraints = HashMap<(CircuitNode, CircuitNode), u64>;

pub fn constraint_cycle_time(hbcn: &HBCN, ct: u64) -> Result<PathConstraints, Box<dyn Error>> {
    let env = Env::new("hbcn_constraining.log")?;
    let mut m = Model::new("constraint", &env)?;

    let factor = m.add_var("factor", Continuous, 0.0, 0.0, INFINITY, &[], &[])?;

    let arr_var: HashMap<NodeIndex, Var> = hbcn
        .node_indices()
        .map(|x| {
            (
                x,
                m.add_var("", Continuous, 0.0, 0.0, INFINITY, &[], &[])
                    .unwrap(),
            )
        })
        .collect();

    let mut delay_vars: HashMap<(&CircuitNode, &CircuitNode), Option<Var>> = hbcn
        .edge_indices()
        .map(|ie| {
            let (src, dst) = hbcn.edge_endpoints(ie).unwrap();

            ((hbcn[src].circuit_node(), hbcn[dst].circuit_node()), None)
        })
        .collect();

    for v in delay_vars.values_mut() {
        *v = Some(m.add_var("", Integer, 0.0, 1.0, INFINITY, &[], &[])?);
    }

    for ie in hbcn.edge_indices() {
        let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
        let place = &hbcn[ie];
        let delay = delay_vars[&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
            .as_ref()
            .unwrap();

        m.add_constr(
            "",
            1.0 * delay + 1.0 * &arr_var[&src] - 1.0 * &arr_var[&dst],
            Equal,
            if place.token { ct as f64 } else { 0.0 },
        )?;

        m.add_constr(
            "",
            1.0 * delay - (place.weight as f64) * &factor,
            Greater,
            0.0,
        )?;
    }

    m.update()?;

    m.set_objective(&factor, Maximize)?;

    m.write("hbcn_constraining.lp")?;

    m.optimize()?;

    match m.status()? {
        Status::Optimal | Status::SubOptimal => Ok(delay_vars
            .into_iter()
            .filter_map(|((src, dst), var)| {
                let val = m.get_values(attr::X, &[var?]).ok()?[0] as u64;
                Some(((src.clone(), dst.clone()), val))
            })
            .collect()),
        _ => Err(Box::new(SolverError::Infeasible)),
    }
}

pub fn compute_cycle_time(hbcn: &HBCN) -> Result<(f64, SolvedHBCN), Box<dyn Error>> {
    let env = Env::new("hbcn_analysis.log")?;
    let mut m = Model::new("analysis", &env)?;
    let cycle_time = m.add_var("cycle_time", Integer, 0.0, 0.0, INFINITY, &[], &[])?;

    let mut arr_var: HashMap<NodeIndex, Var> = hbcn
        .node_indices()
        .map(|x| {
            (
                x,
                m.add_var("", Integer, 0.0, 0.0, INFINITY, &[], &[])
                    .unwrap(),
            )
        })
        .collect();

    let mut slack_var: HashMap<EdgeIndex, Var> = hbcn
        .edge_indices()
        .map(|ie| {
            let (ref src, ref dst) = hbcn.edge_endpoints(ie).unwrap();
            let ref place = hbcn[ie];
            let slack = m
                .add_var("", Integer, 0.0, 0.0, INFINITY, &[], &[])
                .unwrap();

            let mut expr = 1.0 * &arr_var[dst] - 1.0 * &arr_var[src] - 1.0 * &slack;
            if place.token {
                expr += 1.0 * &cycle_time;
            }
            m.add_constr("", expr, Equal, place.weight as f64).unwrap();

            (ie, slack)
        })
        .collect();

    m.update()?;

    m.set_objective(&cycle_time, Minimize)?;

    m.write("hbcn_analysis.lp")?;

    m.optimize()?;
    if m.status()? == Status::InfOrUnbd {
        Err(Box::new(SolverError::Infeasible))
    } else {
        Ok((
            m.get(attr::ObjVal)?,
            hbcn.filter_map(
                |ix, x| {
                    Some(TransitionEvent {
                        transition: x.clone(),
                        time: m
                            .get_values(attr::X, &[arr_var.remove(&ix).unwrap()])
                            .ok()?[0]
                            .round() as u64,
                    })
                },
                |ie, e| {
                    Some(SlackedPlace {
                        place: e.clone(),
                        slack: m
                            .get_values(attr::X, &[slack_var.remove(&ie).unwrap()])
                            .ok()?[0]
                            .round() as u64,
                    })
                },
            ),
        ))
    }
}

pub fn write_vcd(hbcn: &SolvedHBCN, w: &mut dyn io::Write) -> io::Result<()> {
    let mut writer = vcd::Writer::new(w);
    let re = Regex::new(r"[^a-zA-Z0-9_]").unwrap();

    writer.timescale(1, vcd::TimescaleUnit::PS)?;
    writer.add_module("top")?;

    let mut variables = HashMap::new();
    let mut event_heap = BinaryHeap::new();

    for ix in hbcn.node_indices() {
        let ref event = hbcn[ix];
        event_heap.push(event.clone());

        let cnode = event.transition.name();
        if !variables.contains_key(cnode) {
            variables.insert(
                cnode.clone(),
                writer.add_wire(1, &re.replace_all(cnode, "_"))?,
            );
        }
    }

    for (_, var) in variables.iter() {
        writer.change_scalar(*var, vcd::Value::V0)?;
    }

    for (time, events) in event_heap
        .into_sorted_vec()
        .into_iter()
        .group_by(|x| x.time)
        .into_iter()
    {
        writer.timestamp(time)?;
        for event in events {
            match event.transition {
                Transition::Data(id) => writer.change_scalar(variables[id.name()], vcd::Value::V1),
                Transition::Spacer(id) => {
                    writer.change_scalar(variables[id.name()], vcd::Value::V0)
                }
            }?;
        }
    }

    Ok(())
}
