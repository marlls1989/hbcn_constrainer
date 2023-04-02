use super::{
    structural_graph::{Channel, ChannelPhase, CircuitNode, StructuralGraph, Symbol},
    AppError,
};
use gurobi::{attr, ConstrSense::*, Env, Model, ModelSense::*, Status, Var, VarType::*, INFINITY};
use itertools::Itertools;
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    prelude::*,
    stable_graph::StableGraph,
    visit::IntoNodeReferences,
    EdgeDirection,
};
use rayon::prelude::*;
use regex::Regex;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    error::Error,
    fmt, io,
    iter::FromIterator,
};

// this is the most engineery way to compute the ceiling of the log base 2 of a number
fn clog2(x: usize) -> usize {
    const NUM_BITS: usize = (std::mem::size_of::<usize>() as usize) * 8;
    NUM_BITS - (x.leading_zeros() as usize)
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

#[derive(PartialEq, Debug, Clone, Default)]
pub struct Place {
    pub backward: bool,
    pub token: bool,
    pub weight: f64,
    pub is_internal: bool,
    pub relative_endpoints: HashSet<NodeIndex>,
}

pub type HBCN = StableGraph<Transition, Place>;

pub fn from_structural_graph(
    g: &StructuralGraph,
    reflexive: bool,
    forward_completion: bool,
) -> Option<HBCN> {
    let mut ret = HBCN::new();
    struct VertexItem {
        token: NodeIndex,
        spacer: NodeIndex,
        backward_cost: f64,
        forward_cost: f64,
        base_cost: f64,
    }
    let vertice_map: HashMap<NodeIndex, VertexItem> = g
        .node_indices()
        .map(|ix| {
            let val = &g[ix];
            let token = ret.add_node(Transition::Data(val.clone()));
            let spacer = ret.add_node(Transition::Spacer(val.clone()));
            let base_cost = val.base_cost() as f64;
            let backward_cost =
                5.0 + 10.0 * clog2(g.edges_directed(ix, Direction::Outgoing).count()) as f64;
            let forward_cost =
                10.0 * clog2(g.edges_directed(ix, Direction::Incoming).count()) as f64;
            (
                ix,
                VertexItem {
                    token,
                    spacer,
                    backward_cost,
                    forward_cost,
                    base_cost,
                },
            )
        })
        .collect();

    for ix in g.edge_indices() {
        let Channel { is_internal, .. } = g[ix];

        let (ref src, ref dst) = g.edge_endpoints(ix)?;
        let VertexItem {
            token: src_token,
            spacer: src_spacer,
            backward_cost,
            base_cost: src_base_cost,
            ..
        } = vertice_map.get(src)?;
        let VertexItem {
            token: dst_token,
            spacer: dst_spacer,
            forward_cost,
            base_cost: dst_base_cost,
            ..
        } = vertice_map.get(dst)?;
        let Channel {
            initial_phase,
            virtual_delay,
            ..
        } = g[ix];

        let forward_cost = if forward_completion {
            virtual_delay.max(*forward_cost + *src_base_cost)
        } else {
            virtual_delay
        };
        let backward_cost = *backward_cost + *dst_base_cost;

        ret.add_edge(
            *src_token,
            *dst_token,
            Place {
                backward: false,
                token: initial_phase == ChannelPhase::ReqData,
                relative_endpoints: HashSet::new(),
                weight: forward_cost,
                is_internal,
            },
        );
        ret.add_edge(
            *src_spacer,
            *dst_spacer,
            Place {
                backward: false,
                token: initial_phase == ChannelPhase::ReqNull,
                relative_endpoints: HashSet::new(),
                weight: forward_cost,
                is_internal,
            },
        );
        ret.add_edge(
            *dst_token,
            *src_spacer,
            Place {
                backward: true,
                token: initial_phase == ChannelPhase::AckData,
                relative_endpoints: HashSet::new(),
                weight: backward_cost,
                is_internal,
            },
        );
        ret.add_edge(
            *dst_spacer,
            *src_token,
            Place {
                backward: true,
                token: initial_phase == ChannelPhase::AckNull,
                relative_endpoints: HashSet::new(),
                weight: backward_cost,
                is_internal,
            },
        );
    }

    if reflexive {
        // For all nodes ix in g
        for ix in g.node_indices() {
            // get pair of transitions related to ix and the forward CD cost
            let VertexItem {
                token: ix_data,
                spacer: ix_null,
                forward_cost,
                ..
            } = vertice_map.get(&ix)?;

            // Find all predecessors is of ix
            for is in g.neighbors_directed(ix, EdgeDirection::Incoming) {
                // get pair of transitions related to is
                let VertexItem {
                    token: is_data,
                    spacer: is_null,
                    base_cost,
                    ..
                } = vertice_map.get(&is)?;

                // Find all predecessors id of ix
                for id in g.neighbors_directed(ix, EdgeDirection::Incoming) {
                    let VertexItem {
                        token: id_data,
                        spacer: id_null,
                        backward_cost,
                        ..
                    } = vertice_map.get(&id)?;

                    // the cost of the reflexive path is the associated cost of both completion
                    // detection circitry plus an aditional c-element
                    let cost = base_cost + backward_cost + forward_cost + 10.0;

                    // If a path is established between is and id, update Place
                    // Else create a reflexive path between is and id
                    if let Some(ie) = ret.find_edge(*is_data, *id_null) {
                        ret[ie].relative_endpoints.insert(*ix_data);
                        ret[ie].weight = ret[ie].weight.max(cost);
                    } else {
                        ret.add_edge(
                            *is_data,
                            *id_null,
                            Place {
                                token: ret[ret.find_edge(*is_data, *ix_data)?].token
                                    || ret[ret.find_edge(*ix_data, *id_null)?].token,
                                relative_endpoints: HashSet::from_iter([*ix_data]), //set![*ix_data],
                                weight: cost,
                                is_internal: false,
                                backward: false,
                            },
                        );
                    }
                    if let Some(ie) = ret.find_edge(*is_null, *id_data) {
                        ret[ie].relative_endpoints.insert(*ix_null);
                        ret[ie].weight = ret[ie].weight.max(cost);
                    } else {
                        ret.add_edge(
                            *is_null,
                            *id_data,
                            Place {
                                token: ret[ret.find_edge(*is_null, *ix_null)?].token
                                    || ret[ret.find_edge(*ix_null, *id_data)?].token,
                                relative_endpoints: HashSet::from_iter([*ix_null]),
                                weight: cost,
                                is_internal: false,
                                backward: false,
                            },
                        );
                    }
                }
            }
        }
    }

    Some(ret)
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct TransitionEvent {
    pub time: f64,
    pub transition: Transition,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct DelayPair {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct DelayedPlace {
    pub place: Place,
    pub delay: DelayPair,
}

pub type DelayedHBCN = StableGraph<TransitionEvent, DelayedPlace>;

#[derive(Debug, Clone, Default)]
pub struct SlackedPlace {
    pub place: Place,
    pub slack: f64,
}

pub type SlackedHBCN = StableGraph<TransitionEvent, SlackedPlace>;

pub type PathConstraints = HashMap<(CircuitNode, CircuitNode), DelayPair>;

pub type TimedHBCN<T> = StableGraph<TransitionEvent, T>;

#[derive(Debug, Clone)]
pub struct ConstrainerResult {
    pub pseudoclock_period: f64,
    pub hbcn: DelayedHBCN,
    pub path_constraints: PathConstraints,
}

pub fn find_cycles(hbcn: &HBCN, weighted: bool) -> Vec<(usize, Vec<(NodeIndex, NodeIndex)>)> {
    let mut loop_breakers = Vec::new();
    let mut start_points = HashSet::new();

    let filtered_hbcn = hbcn.filter_map(
        |_, x| Some(x.clone()),
        |ie, e| {
            let weight = if weighted { e.weight as f64 } else { 1.0 };
            if e.token {
                let (u, v) = hbcn.edge_endpoints(ie)?;
                loop_breakers.push((u, weight, v));
                start_points.insert(v);
                None
            } else {
                Some(-weight)
            }
        },
    );

    // creates a map with a distance from all start_points to all other nodes
    let bellman_distances: HashMap<NodeIndex, Vec<(f64, Option<NodeIndex>)>> = start_points
        .into_par_iter()
        .map(|ix| {
            let (costs, predecessors) = petgraph::algo::bellman_ford(&filtered_hbcn, ix).unwrap();

            (
                ix,
                // Zips together the distance and predecessor list
                costs.into_iter().zip_eq(predecessors.into_iter()).collect(),
            )
        })
        .collect();

    let mut paths: Vec<(usize, Vec<(NodeIndex, NodeIndex)>)> = loop_breakers
        .into_par_iter()
        .filter_map(|(it, e, is)| {
            let nodes = &bellman_distances[&is];
            let cost = e - nodes[it.index()].0;
            // Recreates the path by traveling the predecessors list
            let path: Vec<_> = {
                let mut current_node = it;
                let mut path = vec![it];
                while current_node != is {
                    if let (_, Some(node)) = nodes[current_node.index()] {
                        path.push(node);
                        current_node = node;
                    } else {
                        return None;
                    }
                }
                path.reverse();

                path.iter()
                    .cloned()
                    .zip(path.iter().skip(1).cloned().chain(std::iter::once(is)))
                    .collect()
            };
            Some((cost as usize, path))
        })
        .collect();

    paths.par_sort_unstable_by_key(|(x, _)| cmp::Reverse(*x));

    paths
}

pub fn constraint_selfreflexive_paths(paths: &mut PathConstraints, val: f64) {
    let nodes: HashSet<CircuitNode> = paths
        .iter()
        .flat_map(move |((src, dst), _)| [src, dst])
        .cloned()
        .collect();

    for n in nodes {
        paths.insert(
            (n.clone(), n),
            DelayPair {
                min: None,
                max: Some(val),
            },
        );
    }
}

pub fn constraint_cycle_time_pseudoclock(
    hbcn: &HBCN,
    ct: f64,
    min_delay: f64,
) -> Result<ConstrainerResult, Box<dyn Error>> {
    assert!(ct > 0.0);

    let env = Env::new("hbcn.log")?;
    let mut m = Model::new("constraint", &env)?;

    let pseudo_clock = m.add_var(
        "pseudo_clock",
        Continuous,
        0.0,
        min_delay,
        INFINITY,
        &[],
        &[],
    )?;

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
        *v = Some(m.add_var("", Continuous, 0.0, min_delay, INFINITY, &[], &[])?);
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
            if place.token { ct } else { 0.0 },
        )?;

        if place.is_internal {
            m.add_constr("", 1.0 * delay, Greater, min_delay)?;
        } else {
            m.add_constr("", 1.0 * delay - 1.0 * &pseudo_clock, Greater, 0.0)?;
        }
    }

    m.update()?;

    m.set_objective(&pseudo_clock, Maximize)?;

    m.optimize()?;

    let pseudo_clock = m.get_values(attr::X, &[pseudo_clock])?[0];

    match m.status()? {
        Status::Optimal | Status::SubOptimal => Ok(ConstrainerResult {
            pseudoclock_period: pseudo_clock,
            path_constraints: delay_vars
                .iter()
                .filter_map(|((src, dst), var)| {
                    m.get_values(attr::X, &[var.clone()?]).ok().map(|x| {
                        (
                            (CircuitNode::clone(src), CircuitNode::clone(dst)),
                            DelayPair {
                                min: None,
                                max: Some(x[0]),
                            },
                        )
                    })
                })
                .collect(),
            hbcn: hbcn.map(
                |ix, x| TransitionEvent {
                    transition: x.clone(),
                    time: m
                        .get_values(attr::X, &[arr_var[&ix].clone()])
                        .ok()
                        .map(|x| x[0])
                        .unwrap_or(0.0),
                },
                |ie, e| {
                    let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
                    DelayedPlace {
                        place: e.clone(),
                        delay: DelayPair {
                            min: None,
                            max: m
                                .get_values(
                                    attr::X,
                                    &[delay_vars
                                        [&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
                                        .clone()
                                        .unwrap()],
                                )
                                .ok()
                                .map(|x| x[0])
                                .filter(|x| (*x - min_delay) / min_delay > 0.001),
                        },
                    }
                },
            ),
        }),
        _ => Err(AppError::Infeasible.into()),
    }
}

pub fn constraint_cycle_time_proportional(
    hbcn: &HBCN,
    ct: f64,
    min_delay: f64,
    backward_margin: Option<f64>,
    forward_margin: Option<f64>,
) -> Result<ConstrainerResult, Box<dyn Error>> {
    assert!(ct > 0.0);
    assert!(min_delay >= 0.0);

    struct DelayVarPair {
        max: Var,
        min: Var,
    }

    let env = Env::new("hbcn.log")?;
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

    let delay_vars: HashMap<(&CircuitNode, &CircuitNode), DelayVarPair> = hbcn
        .edge_indices()
        .map(|ie| {
            let (src, dst) = hbcn.edge_endpoints(ie).unwrap();

            let max = m
                .add_var("", Continuous, 0.0, min_delay, INFINITY, &[], &[])
                .unwrap();
            let min: Var = m
                .add_var("", Continuous, 0.0, 0.0, INFINITY, &[], &[])
                .unwrap();

            (
                (hbcn[src].circuit_node(), hbcn[dst].circuit_node()),
                DelayVarPair { max, min },
            )
        })
        .collect();

    for ie in hbcn.edge_indices() {
        let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
        let place = &hbcn[ie];
        let delay_var = &delay_vars[&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())];

        m.add_constr(
            "",
            1.0 * &delay_var.max + 1.0 * &arr_var[&src] - 1.0 * &arr_var[&dst],
            Equal,
            if place.token { ct } else { 0.0 },
        )?;

        m.add_constr(
            "",
            1.0 * &delay_var.max - place.weight * &factor,
            Greater,
            0.0,
        )?;

        if !place.is_internal {
            if place.backward {
                if forward_margin.is_some() {
                    let matching_delay = delay_vars
                        .get(&(hbcn[dst].circuit_node(), hbcn[src].circuit_node()))
                        .expect("malformed HBCN");
                    m.add_constr(
                        "",
                        1.0 * &delay_var.min - 1.0 * &matching_delay.max
                            + 1.0 * &matching_delay.min,
                        Equal,
                        0.0,
                    )?;
                }
                if let Some(bm) = backward_margin {
                    m.add_constr(
                        "",
                        bm * &delay_var.max - 1.0 * &delay_var.min,
                        if forward_margin.is_some() {
                            Greater
                        } else {
                            Equal
                        },
                        0.0,
                    )?;
                } else if forward_margin.is_some() {
                    m.add_constr(
                        "",
                        1.0 * &delay_var.max - 1.0 * &delay_var.min,
                        Greater,
                        0.0,
                    )?;
                }
            } else if let Some(fm) = forward_margin {
                m.add_constr("", fm * &delay_var.max - 1.0 * &delay_var.min, Equal, 0.0)?;
            }
        }
    }

    m.update()?;

    m.set_objective(&factor, Maximize)?;

    m.optimize()?;

    match m.status()? {
        Status::Optimal | Status::SubOptimal => Ok(ConstrainerResult {
            pseudoclock_period: min_delay,
            path_constraints: delay_vars
                .iter()
                .map(|((src, dst), var)| {
                    let min = m
                        .get_values(attr::X, &[var.min.clone()])
                        .ok()
                        .map(|x| x[0])
                        .filter(|x| *x > 0.001);
                    let max = m
                        .get_values(attr::X, &[var.max.clone()])
                        .ok()
                        .map(|x| x[0])
                        .filter(|x| (*x - min_delay) / min_delay > 0.001);
                    (
                        (CircuitNode::clone(src), CircuitNode::clone(dst)),
                        DelayPair { min, max },
                    )
                })
                .collect(),
            hbcn: hbcn.map(
                |ix, x| TransitionEvent {
                    transition: x.clone(),
                    time: m
                        .get_values(attr::X, &[arr_var[&ix].clone()])
                        .ok()
                        .map(|x| x[0])
                        .unwrap_or(0.0),
                },
                |ie, e| {
                    let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
                    DelayedPlace {
                        place: e.clone(),
                        delay: DelayPair {
                            min: m
                                .get_values(
                                    attr::X,
                                    &[delay_vars
                                        [&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
                                        .min
                                        .clone()],
                                )
                                .ok()
                                .map(|x| x[0])
                                .filter(|x| *x > 0.001),
                            max: m
                                .get_values(
                                    attr::X,
                                    &[delay_vars
                                        [&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
                                        .max
                                        .clone()],
                                )
                                .ok()
                                .map(|x| x[0])
                                .filter(|x| (*x - min_delay) / min_delay > 0.001),
                        },
                    }
                },
            ),
        }),
        _ => Err(AppError::Infeasible.into()),
    }
}

pub fn compute_cycle_time(hbcn: &HBCN) -> Result<(f64, SlackedHBCN), Box<dyn Error>> {
    let env = Env::new("hbcn.log")?;
    let mut m = Model::new("analysis", &env)?;
    let cycle_time = m.add_var("cycle_time", Integer, 0.0, 0.0, INFINITY, &[], &[])?;

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

    let slack_var: HashMap<EdgeIndex, Var> = hbcn
        .edge_indices()
        .map(|ie| {
            let (ref src, ref dst) = hbcn.edge_endpoints(ie).unwrap();
            let place = &hbcn[ie];
            let slack = m
                .add_var("", Continuous, 0.0, 0.0, INFINITY, &[], &[])
                .unwrap();

            m.add_constr(
                "",
                1.0 * &arr_var[dst] - 1.0 * &arr_var[src] - 1.0 * &slack
                    + if place.token { 1.0 } else { 0.0 } * &cycle_time,
                Equal,
                place.weight as f64,
            )
            .unwrap();

            (ie, slack)
        })
        .collect();

    m.update()?;

    m.set_objective(&cycle_time, Minimize)?;

    m.optimize()?;
    if m.status()? == Status::InfOrUnbd {
        Err(AppError::Infeasible.into())
    } else {
        Ok((
            m.get(attr::ObjVal)?,
            hbcn.filter_map(
                |ix, x| {
                    Some(TransitionEvent {
                        transition: x.clone(),
                        time: m.get_values(attr::X, &[arr_var[&ix].clone()]).ok()?[0],
                    })
                },
                |ie, e| {
                    Some(SlackedPlace {
                        place: e.clone(),
                        slack: m.get_values(attr::X, &[slack_var[&ie].clone()]).ok()?[0],
                        ..Default::default()
                    })
                },
            ),
        ))
    }
}

pub fn write_vcd<T>(hbcn: &TimedHBCN<T>, w: &mut dyn io::Write) -> io::Result<()> {
    let mut writer = vcd::Writer::new(w);
    let re = Regex::new(r"[^a-zA-Z0-9_]").unwrap();

    writer.timescale(1, vcd::TimescaleUnit::PS)?;
    writer.add_module("top")?;

    let mut variables = HashMap::new();

    let events = {
        let mut events: Vec<&TransitionEvent> = hbcn
            .node_references()
            .map(|(_idx, e)| {
                let cnode = e.transition.name();
                if !variables.contains_key(cnode) {
                    variables.insert(
                        cnode.clone(),
                        writer.add_wire(1, &re.replace_all(cnode, "_")).unwrap(),
                    );
                }

                e
            })
            .collect();
        events.par_sort_unstable_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        events
    };

    for (_, var) in variables.iter() {
        writer.change_scalar(*var, vcd::Value::V0)?;
    }

    for (time, events) in events.into_iter().group_by(|x| x.time).into_iter() {
        writer.timestamp((time.abs() * 1000.0) as u64)?;
        for event in events {
            match &event.transition {
                Transition::Data(id) => writer.change_scalar(variables[id.name()], vcd::Value::V1),
                Transition::Spacer(id) => {
                    writer.change_scalar(variables[id.name()], vcd::Value::V0)
                }
            }?;
        }
    }

    Ok(())
}
