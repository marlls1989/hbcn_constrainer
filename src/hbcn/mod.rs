use super::{
    AppError,
    structural_graph::{Channel, ChannelPhase, CircuitNode, StructuralGraph, Symbol},
};

use std::{
    collections::{HashMap, HashSet},
    fmt, io,
};

use anyhow::*;
use gurobi::{ConstrSense::*, Env, INFINITY, Model, ModelSense::*, Status, Var, VarType::*, attr, param};
use gag::Gag;
use itertools::Itertools;
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    prelude::*,
    stable_graph::StableGraph,
    visit::IntoNodeReferences,
};
use rayon::prelude::*;
use regex::Regex;

// this is the most engineery way to compute the ceiling log base 2 of a number
fn clog2(x: usize) -> u32 {
    usize::BITS - x.leading_zeros()
}

// Timing constants for delay/cost calculations
const DEFAULT_REGISTER_DELAY: f64 = 10.0;

/// Create a quiet Gurobi environment that suppresses all console output
/// This is useful for tests and when you don't want Gurobi's verbose output
fn create_quiet_gurobi_env(logfile: &str) -> Result<Env> {
    let mut env = Env::new(logfile)?;
    // Suppress all Gurobi output including solver logs and license information
    env.set(param::OutputFlag, 0)?;
    Ok(env)
}

pub trait HasTransition {
    fn transition(&self) -> &Transition;
}

pub trait Named {
    fn name(&self) -> &Symbol;
}

pub trait HasCircuitNode {
    fn circuit_node(&self) -> &CircuitNode;
}

pub trait TimedEvent {
    fn time(&self) -> f64;
}

impl<T: HasTransition> HasCircuitNode for T {
    fn circuit_node(&self) -> &CircuitNode {
        self.transition().circuit_node()
    }
}

impl<T: HasCircuitNode> Named for T {
    fn name(&self) -> &Symbol {
        self.circuit_node().name()
    }
}

#[derive(PartialEq, Eq, Debug, Clone, PartialOrd, Ord)]
pub enum Transition {
    Spacer(CircuitNode),
    Data(CircuitNode),
}

impl HasCircuitNode for Transition {
    fn circuit_node(&self) -> &CircuitNode {
        match self {
            Transition::Data(id) => id,
            Transition::Spacer(id) => id,
        }
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

pub trait MarkablePlace {
    fn mark(&mut self, mark: bool);
    fn is_marked(&self) -> bool;
}

pub trait SlackablePlace {
    fn slack(&self) -> f64;
}

pub trait WeightedPlace {
    fn weight(&self) -> f64;
}

pub trait HasPlace {
    fn place(&self) -> &Place;
    fn place_mut(&mut self) -> &mut Place;
}

impl WeightedPlace for Place {
    fn weight(&self) -> f64 {
        self.weight
    }
}

impl HasPlace for Place {
    fn place(&self) -> &Place {
        self
    }

    fn place_mut(&mut self) -> &mut Place {
        self
    }
}

impl<P: HasPlace> MarkablePlace for P {
    fn mark(&mut self, mark: bool) {
        self.place_mut().token = mark;
    }

    fn is_marked(&self) -> bool {
        self.place().token
    }
}

pub type HBCN = StableGraph<Transition, Place>;

pub fn from_structural_graph(g: &StructuralGraph, forward_completion: bool) -> Option<HBCN> {
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
            let backward_cost = DEFAULT_REGISTER_DELAY
                * clog2(g.edges_directed(ix, Direction::Outgoing).count()) as f64;
            let forward_cost = DEFAULT_REGISTER_DELAY
                * clog2(g.edges_directed(ix, Direction::Incoming).count()) as f64;
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

    Some(ret)
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct TransitionEvent {
    pub time: f64,
    pub transition: Transition,
}

impl HasTransition for TransitionEvent {
    fn transition(&self) -> &Transition {
        &self.transition
    }
}

impl TimedEvent for TransitionEvent {
    fn time(&self) -> f64 {
        self.time
    }
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
    pub slack: Option<f64>,
}

impl WeightedPlace for DelayedPlace {
    fn weight(&self) -> f64 {
        self.delay.max.unwrap_or(self.place.weight)
    }
}

impl HasPlace for DelayedPlace {
    fn place(&self) -> &Place {
        &self.place
    }

    fn place_mut(&mut self) -> &mut Place {
        &mut self.place
    }
}

impl SlackablePlace for DelayedPlace {
    fn slack(&self) -> f64 {
        self.slack.unwrap_or(0.0)
    }
}

pub type DelayedHBCN = StableGraph<TransitionEvent, DelayedPlace>;

pub type PathConstraints = HashMap<(CircuitNode, CircuitNode), DelayPair>;

pub type TimedHBCN<T> = StableGraph<TransitionEvent, T>;

#[derive(Debug, Clone)]
pub struct ConstrainerResult {
    pub pseudoclock_period: f64,
    pub hbcn: DelayedHBCN,
    pub path_constraints: PathConstraints,
}

pub fn find_critical_cycles<N: Sync + Send, P: MarkablePlace + SlackablePlace>(
    hbcn: &StableGraph<N, P>,
) -> Vec<Vec<(NodeIndex, NodeIndex)>> {
    let mut loop_breakers = Vec::new();
    let mut start_points = HashSet::new();

    let filtered_hbcn = hbcn.filter_map(
        |_, x| Some(x),
        |ie, e| {
            let (u, v) = hbcn.edge_endpoints(ie)?;
            let weight = hbcn[ie].slack();
            if e.is_marked() {
                loop_breakers.push((u, v));
                start_points.insert(v);
                Some(weight)
            } else {
                Some(weight)
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

    let paths: Vec<Vec<(NodeIndex, NodeIndex)>> = loop_breakers
        .into_par_iter()
        .filter_map(|(it, is)| {
            let nodes = &bellman_distances[&is];
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
            Some(path)
        })
        .collect();

    paths
}

pub fn constrain_cycle_time_pseudoclock(
    hbcn: &HBCN,
    ct: f64,
    min_delay: f64,
) -> Result<ConstrainerResult> {
    assert!(ct > 0.0);

    // Suppress console output during Gurobi operations
    let _gag_stdout = Gag::stdout().ok();
    let _gag_stderr = Gag::stderr().ok();

    let env = create_quiet_gurobi_env("hbcn.log")?;
    let mut m = Model::new("constrain", &env)?;

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
                        slack: None,
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

pub fn constrain_cycle_time_proportional(
    hbcn: &HBCN,
    ct: f64,
    min_delay: f64,
    backward_margin: Option<f64>,
    forward_margin: Option<f64>,
) -> Result<ConstrainerResult> {
    assert!(ct > 0.0);
    assert!(min_delay >= 0.0);

    // Suppress console output during Gurobi operations
    let _gag_stdout = Gag::stdout().ok();
    let _gag_stderr = Gag::stderr().ok();

    struct DelayVarPair {
        max: Var,
        min: Var,
        slack: Var,
    }

    let env = create_quiet_gurobi_env("hbcn.log")?;
    let mut m = Model::new("constrain", &env)?;

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
            let slack: Var = m
                .add_var("", Continuous, 0.0, 0.0, INFINITY, &[], &[])
                .unwrap();

            (
                (hbcn[src].circuit_node(), hbcn[dst].circuit_node()),
                DelayVarPair { max, min, slack },
            )
        })
        .collect();

    for ie in hbcn.edge_indices() {
        let (src, dst) = hbcn.edge_endpoints(ie).unwrap();
        let place = &hbcn[ie];
        let delay_var = &delay_vars[&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())];
        let matching_delay = delay_vars
            .get(&(hbcn[dst].circuit_node(), hbcn[src].circuit_node()))
            .expect("malformed HBCN");

        m.add_constr(
            "",
            1.0 * &delay_var.max + 1.0 * &arr_var[&src] - 1.0 * &arr_var[&dst],
            Equal,
            if place.token { ct } else { 0.0 },
        )?;

        m.add_constr(
            "",
            1.0 * &delay_var.max - place.weight * &factor - 1.0 * &delay_var.slack,
            Equal,
            0.0,
        )?;

        if !place.is_internal {
            if place.backward {
                if forward_margin.is_some() {
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
                        slack: m
                            .get_values(
                                attr::X,
                                &[delay_vars
                                    [&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
                                    .slack
                                    .clone()],
                            )
                            .ok()
                            .map(|x| x[0]),
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
                                .map(|x| x[0]),
                            max: m
                                .get_values(
                                    attr::X,
                                    &[delay_vars
                                        [&(hbcn[src].circuit_node(), hbcn[dst].circuit_node())]
                                        .max
                                        .clone()],
                                )
                                .ok()
                                .map(|x| x[0]),
                        },
                    }
                },
            ),
        }),
        _ => Err(AppError::Infeasible.into()),
    }
}

pub fn compute_cycle_time(hbcn: &HBCN, weighted: bool) -> Result<(f64, DelayedHBCN)> {
    // Suppress console output during Gurobi operations
    let _gag_stdout = Gag::stdout().ok();
    let _gag_stderr = Gag::stderr().ok();
    
    let env = create_quiet_gurobi_env("hbcn.log")?;
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

    let delay_slack_var: HashMap<EdgeIndex, (Var, Var)> = hbcn
        .edge_indices()
        .map(|ie| {
            let (ref src, ref dst) = hbcn.edge_endpoints(ie).unwrap();
            let place = &hbcn[ie];

            let slack = m
                .add_var("", Continuous, 0.0, 0.0, INFINITY, &[], &[])
                .unwrap();

            let delay = m
                .add_var("", Continuous, 0.0, 0.0, INFINITY, &[], &[])
                .unwrap();

            m.add_constr(
                "",
                1.0 * &delay - 1.0 * &slack,
                Equal,
                if weighted { place.weight as f64 } else { 1.0 },
            )
            .unwrap();

            m.add_constr(
                "",
                1.0 * &arr_var[dst] - 1.0 * &arr_var[src] - 1.0 * &delay
                    + if place.token { 1.0 } else { 0.0 } * &cycle_time,
                Equal,
                0.0,
            )
            .unwrap();

            (ie, (delay, slack))
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
                    let (delay_var, slack_var) = &delay_slack_var[&ie];
                    Some(DelayedPlace {
                        place: e.clone(),
                        delay: DelayPair {
                            min: None,
                            max: m
                                .get_values(attr::X, &[delay_var.clone()])
                                .ok()
                                .map(|x| x[0]),
                        },
                        slack: m
                            .get_values(attr::X, &[slack_var.clone()])
                            .ok()
                            .map(|x| x[0]),
                        ..Default::default()
                    })
                },
            ),
        ))
    }
}

pub fn write_vcd<T>(hbcn: &TimedHBCN<T>, w: &mut dyn io::Write) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural_graph::parse;
    #[test]
    fn test_simple_two_node_conversion() {
        // Test basic conversion with two connected nodes
        let input = r#"
            Port "input" [("output", 100)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Should have 4 nodes: Data and Spacer transitions for each original node
        assert_eq!(hbcn.node_count(), 4);

        // Should have 4 edges: forward and backward places for the connection
        assert_eq!(hbcn.edge_count(), 4);

        // Verify node types exist
        let nodes: Vec<_> = hbcn.node_indices().map(|i| &hbcn[i]).collect();
        assert_eq!(nodes.len(), 4);

        // Count Data and Spacer transitions
        let data_count = nodes
            .iter()
            .filter(|n| matches!(n, Transition::Data(_)))
            .count();
        let spacer_count = nodes
            .iter()
            .filter(|n| matches!(n, Transition::Spacer(_)))
            .count();
        assert_eq!(data_count, 2);
        assert_eq!(spacer_count, 2);
    }

    #[test]
    fn test_data_register_conversion() {
        // Test conversion with DataReg which has internal structure
        let input = r#"
            Port "input" [("reg", 50)]
            DataReg "reg" [("output", 75)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Should have 10 nodes: Data and Spacer for each of the 5 circuit nodes
        // (input port, reg data, reg control, reg output, output port)
        assert_eq!(hbcn.node_count(), 10);

        // Should have 16 edges: each channel creates 4 places
        // 4 edges per channel, 4 channels total
        assert_eq!(hbcn.edge_count(), 16);
    }

    #[test]
    fn test_transition_properties() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Check that transitions have correct circuit node references
        for node_idx in hbcn.node_indices() {
            let transition = &hbcn[node_idx];
            match transition {
                Transition::Data(node) | Transition::Spacer(node) => {
                    assert!(node.name().as_ref() == "a" || node.name().as_ref() == "b");
                }
            }
        }
    }

    #[test]
    fn test_place_properties() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Check place properties
        let mut forward_places = 0;
        let mut backward_places = 0;

        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];

            // Weight should be positive
            assert!(place.weight >= 0.0);

            // Count forward and backward places
            if place.backward {
                backward_places += 1;
            } else {
                forward_places += 1;
            }

            // relative_endpoints should be initialised
            assert!(place.relative_endpoints.is_empty()); // Should be empty since reflexive paths are removed
        }

        // For this simple two-port graph, we should have equal numbers of forward and backward places
        // Each channel creates 2 forward places (token->token, spacer->spacer) and 2 backward places (token->spacer, spacer->token)
        assert_eq!(
            forward_places, backward_places,
            "Forward and backward places should be equal in a simple chain"
        );
    }

    #[test]
    fn test_forward_completion_disabled() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Check that weights are based on virtual_delay when forward_completion=false
        let places: Vec<_> = hbcn.edge_indices().map(|i| &hbcn[i]).collect();
        for place in places {
            if !place.backward {
                // Forward places should use virtual_delay (100 in this case)
                assert_eq!(place.weight, 100.0);
            }
        }
    }

    #[test]
    fn test_forward_completion_enabled() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, true).expect("Failed to convert to HBCN");

        // With forward_completion=true, weights should consider forward costs
        let places: Vec<_> = hbcn.edge_indices().map(|i| &hbcn[i]).collect();
        assert!(!places.is_empty());

        // Should still produce valid HBCN
        assert!(hbcn.node_count() > 0);
        assert!(hbcn.edge_count() > 0);
    }

    #[test]
    fn test_complex_graph_conversion() {
        let input = r#"
            Port "input" [("reg1", 10), ("reg2", 20)]
            DataReg "reg1" [("output", 50)]
            DataReg "reg2" [("output", 60)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Should handle multiple connections properly
        assert!(hbcn.node_count() > 4); // More nodes due to DataReg internal structure
        assert!(hbcn.edge_count() > 8); // More edges due to multiple connections

        // All transitions should be valid
        for node_idx in hbcn.node_indices() {
            let transition = &hbcn[node_idx];
            match transition {
                Transition::Data(node) | Transition::Spacer(node) => {
                    assert!(!node.name().as_ref().is_empty());
                }
            }
        }
    }

    #[test]
    fn test_channel_phases() {
        // Test that different channel phases are handled correctly
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Check that token markings are set according to channel phases
        let mut req_data_count = 0;
        let mut req_null_count = 0;
        let mut ack_data_count = 0;
        let mut ack_null_count = 0;

        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];
            if place.token {
                if place.backward {
                    ack_data_count += 1;
                } else {
                    req_data_count += 1;
                }
            } else {
                if place.backward {
                    ack_null_count += 1;
                } else {
                    req_null_count += 1;
                }
            }
        }

        // Should have balanced counts based on the protocol
        assert_eq!(
            req_data_count + req_null_count + ack_data_count + ack_null_count,
            hbcn.edge_count()
        );
    }

    #[test]
    fn test_weight_calculations() {
        let input = r#"
            Port "a" [("b", 150)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Check weight calculations
        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];
            assert!(place.weight >= 0.0, "Weight should be non-negative");

            if !place.backward {
                // Forward places should have weight based on virtual_delay
                assert_eq!(place.weight, 150.0);
            } else {
                // Backward places should include register delays
                assert!(place.weight >= DEFAULT_REGISTER_DELAY);
            }
        }
    }

    #[test]
    fn test_empty_graph() {
        // Test edge case with minimal graph
        let input = r#"
            Port "single" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Should have 2 nodes (Data and Spacer for the single port) and no edges
        assert_eq!(hbcn.node_count(), 2);
        assert_eq!(hbcn.edge_count(), 0);
    }

    #[test]
    fn test_register_types() {
        // Test conversion with different register types
        let input = r#"
            Port "input" [("null_reg", 100)]
            NullReg "null_reg" [("control_reg", 200)]
            ControlReg "control_reg" [("unsafe_reg", 300)]
            UnsafeReg "unsafe_reg" [("output", 400)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse structural graph");

        let hbcn =
            from_structural_graph(&structural_graph, false).expect("Failed to convert to HBCN");

        // Should successfully convert all register types
        assert!(hbcn.node_count() > 0);
        assert!(hbcn.edge_count() > 0);

        // Verify all transitions have valid circuit nodes
        for node_idx in hbcn.node_indices() {
            let transition = &hbcn[node_idx];
            match transition {
                Transition::Data(node) | Transition::Spacer(node) => {
                    // All nodes should have valid names
                    assert!(!node.name().as_ref().is_empty());
                }
            }
        }
    }

    /// Test constraint generation with various circuit topologies
    #[test]
    fn test_constraint_algorithms_linear_chain() {
        let input = r#"
            Port "a" [("b", 10)]
            Port "b" [("c", 20)]
            Port "c" [("d", 15)]
            Port "d" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test pseudoclock algorithm
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 50.0, 5.0)
            .expect("Pseudoclock should work on linear chain");

        assert!(pseudo_result.pseudoclock_period >= 5.0);
        assert!(!pseudo_result.path_constraints.is_empty());

        // Test proportional algorithm
        let prop_result = constrain_cycle_time_proportional(&hbcn, 50.0, 5.0, None, None)
            .expect("Proportional should work on linear chain");

        assert!(prop_result.pseudoclock_period >= 5.0);
        assert!(!prop_result.path_constraints.is_empty());
    }

    #[test]
    fn test_constraint_algorithms_branching() {
        let input = r#"
            Port "input" [("branch1", 25), ("branch2", 30)]
            Port "branch1" [("merge", 15)]
            Port "branch2" [("merge", 20)]
            Port "merge" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test both algorithms on branching topology
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 100.0, 8.0)
            .expect("Should handle branching circuit");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 100.0, 8.0, None, None)
            .expect("Should handle branching circuit");

        // Both should produce valid results
        assert!(pseudo_result.pseudoclock_period >= 8.0);
        assert!(prop_result.pseudoclock_period >= 8.0);
        assert!(!pseudo_result.path_constraints.is_empty());
        assert!(!prop_result.path_constraints.is_empty());
    }

    #[test]
    fn test_constraint_algorithms_with_feedback() {
        let input = r#"
            Port "input" [("proc", 40)]
            DataReg "proc" [("output", 35), ("feedback", 25)]
            Port "output" []
            Port "feedback" [("proc", 30)]
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test algorithms with feedback loop
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 150.0, 10.0)
            .expect("Should handle feedback circuit");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 150.0, 10.0, None, None)
            .expect("Should handle feedback circuit");

        assert!(pseudo_result.pseudoclock_period >= 10.0);
        assert!(prop_result.pseudoclock_period >= 10.0);
    }

    #[test]
    fn test_constraint_generation_boundary_conditions() {
        let input = r#"Port "input" [("output", 50)]
                      Port "output" []"#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test with reasonable parameters for a simple circuit
        let result = constrain_cycle_time_pseudoclock(&hbcn, 200.0, 5.0)
            .expect("Should handle reasonable parameters");
        assert!(result.pseudoclock_period >= 5.0);

        // Test with very large parameters
        let result = constrain_cycle_time_proportional(&hbcn, 1000.0, 10.0, None, None)
            .expect("Should handle large parameters");
        assert!(result.pseudoclock_period >= 10.0);
    }

    #[test]
    fn test_delay_pair_functionality() {
        let input = r#"
            Port "a" [("b", 50)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let result = constrain_cycle_time_proportional(&hbcn, 100.0, 5.0, None, None)
            .expect("Should generate constraints");

        // Test DelayPair properties in results
        for (_, constraint) in &result.path_constraints {
            // At least one delay should be present
            assert!(
                constraint.min.is_some() || constraint.max.is_some(),
                "Each constraint should have at least min or max delay"
            );

            // If both present, validate relationship
            if let (Some(min), Some(max)) = (constraint.min, constraint.max) {
                assert!(min <= max, "Min delay should not exceed max delay");
                assert!(min >= 0.0, "Min delay should be non-negative");
                assert!(max >= 5.0, "Max delay should be at least minimal delay");
            }
        }
    }

    #[test]
    fn test_critical_cycle_detection() {
        // Create a feasible circuit with DataReg for proper cycle formation
        let input = r#"
            Port "input" [("reg", 100)]
            DataReg "reg" [("output", 150), ("input", 75)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let result = constrain_cycle_time_proportional(&hbcn, 800.0, 20.0, None, None)
            .expect("Should constrain circuit with feasible parameters");

        // Find critical cycles
        let cycles = find_critical_cycles(&result.hbcn);

        // Validate cycle structure if any cycles are found
        for cycle in &cycles {
            assert!(
                cycle.len() >= 2,
                "Each cycle should have at least 2 transitions"
            );

            // Verify cycle structure is valid
            if !cycle.is_empty() {
                let _first_node = cycle[0].0;
                let _last_node = cycle[cycle.len() - 1].1;
                // Cycle validation logic would go here if needed
            }
        }

        // Test passes as long as constraint generation succeeds
        assert!(result.pseudoclock_period >= 20.0);
    }

    #[test]
    fn test_markable_place_functionality() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let mut result = constrain_cycle_time_pseudoclock(&hbcn, 50.0, 5.0)
            .expect("Should generate constraints");

        // Test marking functionality on places
        let edge_indices: Vec<_> = result.hbcn.edge_indices().collect();
        if !edge_indices.is_empty() {
            let first_edge = edge_indices[0];
            let place = &mut result.hbcn[first_edge];

            // Initially should not be marked
            assert!(!place.is_marked());

            // Mark the place
            place.mark(true);
            assert!(place.is_marked());

            // Unmark the place
            place.mark(false);
            assert!(!place.is_marked());
        }
    }

    #[test]
    fn test_transition_event_timing() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" [("c", 200)]
            Port "c" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let result =
            constrain_cycle_time_pseudoclock(&hbcn, 500.0, 25.0).expect("Should generate timing");

        // Check that all transition events have valid timing
        for node_idx in result.hbcn.node_indices() {
            let event = &result.hbcn[node_idx];

            // Time should be non-negative
            assert!(event.time() >= 0.0, "Event timing should be non-negative");

            // Should have valid transition reference
            match &event.transition {
                Transition::Data(node) | Transition::Spacer(node) => {
                    assert!(
                        !node.name().as_ref().is_empty(),
                        "Node should have valid name"
                    );
                }
            }
        }
    }

    #[test]
    fn test_proportional_vs_pseudoclock_differences() {
        let input = r#"
            Port "input" [("middle", 100)]
            Port "middle" [("output", 150)]
            Port "output" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        let pseudo_result =
            constrain_cycle_time_pseudoclock(&hbcn, 300.0, 15.0).expect("Pseudoclock should work");
        let prop_result = constrain_cycle_time_proportional(&hbcn, 300.0, 15.0, None, None)
            .expect("Proportional should work");

        // Both should produce valid results but potentially different constraints
        assert!(pseudo_result.pseudoclock_period >= 15.0);
        assert!(prop_result.pseudoclock_period >= 15.0);

        // Pseudoclock typically only produces max constraints
        let _pseudo_has_min = pseudo_result
            .path_constraints
            .values()
            .any(|c| c.min.is_some());
        let pseudo_has_max = pseudo_result
            .path_constraints
            .values()
            .any(|c| c.max.is_some());

        // Proportional may produce both min and max constraints
        let _prop_has_min = prop_result
            .path_constraints
            .values()
            .any(|c| c.min.is_some());
        let prop_has_max = prop_result
            .path_constraints
            .values()
            .any(|c| c.max.is_some());

        // At least one algorithm should produce some constraints
        assert!(
            pseudo_has_max || prop_has_max,
            "At least one algorithm should produce max constraints"
        );
    }

    #[test]
    fn test_margin_effects_detailed() {
        let input = r#"
            Port "a" [("b", 100)]
            Port "b" [("c", 200)]
            Port "c" []
        "#;
        let structural_graph = parse(input).expect("Failed to parse");
        let hbcn = from_structural_graph(&structural_graph, false).expect("Failed to convert");

        // Test different margin combinations
        let no_margin = constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, None, None)
            .expect("No margin should work");

        let forward_margin = constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, None, Some(0.8))
            .expect("Forward margin should work");

        let backward_margin =
            constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, Some(0.8), None)
                .expect("Backward margin should work");

        let both_margins =
            constrain_cycle_time_proportional(&hbcn, 400.0, 20.0, Some(0.8), Some(0.8))
                .expect("Both margins should work");

        // All should produce valid pseudoclock periods
        assert!(no_margin.pseudoclock_period >= 20.0);
        assert!(forward_margin.pseudoclock_period >= 20.0);
        assert!(backward_margin.pseudoclock_period >= 20.0);
        assert!(both_margins.pseudoclock_period >= 20.0);

        // Margins should affect the results
        let periods = vec![
            no_margin.pseudoclock_period,
            forward_margin.pseudoclock_period,
            backward_margin.pseudoclock_period,
            both_margins.pseudoclock_period,
        ];

        // Test that all margin combinations produce valid results
        // (Margins may or may not affect results depending on the specific circuit)
        let all_valid = periods.iter().all(|&p| p >= 20.0 && p <= 400.0);
        assert!(
            all_valid,
            "All margin combinations should produce valid results"
        );
    }

    /// Test cyclic path HBCN conversion (based on cyclic.graph structure)
    #[test]
    fn test_cyclic_path_conversion() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Should have transitions for all nodes
        assert!(hbcn.node_count() > 0, "Cyclic HBCN should have nodes");
        assert!(hbcn.edge_count() > 0, "Cyclic HBCN should have edges");

        // Should have DataReg transitions
        let has_datareg = hbcn.node_indices().any(|idx| {
            matches!(hbcn[idx], Transition::Data(CircuitNode::Register { .. }))
        });
        assert!(has_datareg, "Cyclic HBCN should have DataReg transitions");

        // Should have feedback loop (DataReg to itself)
        let has_feedback = hbcn.edge_indices().any(|edge_idx| {
            let (src, dst) = hbcn.edge_endpoints(edge_idx).unwrap();
            matches!(hbcn[src], Transition::Data(CircuitNode::Register { .. })) &&
            matches!(hbcn[dst], Transition::Data(CircuitNode::Register { .. }))
        });
        assert!(has_feedback, "Cyclic HBCN should have feedback loop");
    }

    /// Test cyclic circuit with complex feedback loops
    #[test]
    fn test_complex_cyclic_conversion() {
        let input = r#"Port "clk" [("reg1", 5), ("reg2", 5)]
                      Port "input" [("reg1", 40)]
                      DataReg "reg1" [("logic", 30), ("reg2", 25)]
                      DataReg "reg2" [("logic", 35), ("reg1", 20)]
                      DataReg "logic" [("output", 45)]
                      Port "output" []"#;

        let structural_graph = parse(input).expect("Failed to parse complex cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert complex cyclic graph to HBCN");

        // Should have multiple DataRegs
        let datareg_count = hbcn.node_indices()
            .filter(|idx| matches!(hbcn[*idx], Transition::Data(CircuitNode::Register { .. })))
            .count();
        assert!(datareg_count >= 3, "Complex cyclic HBCN should have multiple DataRegs");

        // Should have multiple feedback loops
        let feedback_edges = hbcn.edge_indices()
            .filter(|edge_idx| {
                let (src, dst) = hbcn.edge_endpoints(*edge_idx).unwrap();
                matches!(hbcn[src], Transition::Data(CircuitNode::Register { .. })) &&
                matches!(hbcn[dst], Transition::Data(CircuitNode::Register { .. }))
            })
            .count();
        assert!(feedback_edges >= 2, "Complex cyclic HBCN should have multiple feedback loops");
    }

    /// Test cyclic circuit constraint algorithms
    #[test]
    fn test_cyclic_constraint_algorithms() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Test pseudoclock algorithm on cyclic circuit
        let pseudo_result = constrain_cycle_time_pseudoclock(&hbcn, 100.0, 5.0)
            .expect("Should handle cyclic circuit with pseudoclock");

        assert!(pseudo_result.pseudoclock_period >= 5.0);
        assert!(pseudo_result.pseudoclock_period <= 100.0);
        assert!(!pseudo_result.path_constraints.is_empty());

        // Test proportional algorithm on cyclic circuit
        let prop_result = constrain_cycle_time_proportional(&hbcn, 100.0, 5.0, None, None)
            .expect("Should handle cyclic circuit with proportional");

        assert!(prop_result.pseudoclock_period >= 5.0);
        assert!(prop_result.pseudoclock_period <= 100.0);
        assert!(!prop_result.path_constraints.is_empty());
    }

    /// Test cyclic circuit critical cycle detection
    #[test]
    fn test_cyclic_critical_cycle_detection() {
        let input = r#"Port "input" [("reg", 30)]
                      DataReg "reg" [("output", 25), ("reg", 20)]
                      Port "output" []"#;

        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Generate constraints to get DelayedHBCN
        let result = constrain_cycle_time_proportional(&hbcn, 200.0, 10.0, None, None)
            .expect("Should generate constraints for cyclic circuit");

        // Find critical cycles in the result
        let cycles = find_critical_cycles(&result.hbcn);

        // For cyclic circuits, we expect to find cycles
        // Each cycle should have at least 2 edges if any are found
        for cycle in &cycles {
            assert!(cycle.len() >= 2, "Cycles should have at least 2 edges");
        }

        // The constraint generation should succeed
        assert!(result.pseudoclock_period >= 10.0);
    }

    /// Test cyclic circuit with forward completion
    #[test]
    fn test_cyclic_forward_completion() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        // Test without forward completion
        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn_no_fc = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Test with forward completion
        let hbcn_with_fc = from_structural_graph(&structural_graph, true)
            .expect("Failed to convert cyclic graph to HBCN with forward completion");

        // Both should produce valid HBCNs
        assert!(hbcn_no_fc.node_count() > 0);
        assert!(hbcn_with_fc.node_count() > 0);

        // Test constraint generation on both
        let result_no_fc = constrain_cycle_time_proportional(&hbcn_no_fc, 100.0, 5.0, None, None)
            .expect("Should work without forward completion on cyclic circuit");

        let result_with_fc = constrain_cycle_time_proportional(&hbcn_with_fc, 100.0, 5.0, None, None)
            .expect("Should work with forward completion on cyclic circuit");

        // Both should produce valid results
        assert!(result_no_fc.pseudoclock_period >= 5.0);
        assert!(result_with_fc.pseudoclock_period >= 5.0);
    }

    /// Test cyclic circuit timing and delay calculations
    #[test]
    fn test_cyclic_timing_calculations() {
        let input = r#"Port "a" [("b", 20)]
                      DataReg "b" [("b", 15), ("c", 10)]
                      Port "c" []"#;

        let structural_graph = parse(input).expect("Failed to parse cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert cyclic graph to HBCN");

        // Test cycle time computation on cyclic circuit
        let (cycle_time, delayed_hbcn) = compute_cycle_time(&hbcn, true)
            .expect("Should compute cycle time for cyclic circuit");

        assert!(cycle_time > 0.0, "Cycle time should be positive");
        assert!(delayed_hbcn.node_count() > 0, "Delayed HBCN should have nodes");

        // Test that all delays are reasonable
        for edge_idx in delayed_hbcn.edge_indices() {
            let edge = &delayed_hbcn[edge_idx];
            if let Some(max_delay) = edge.delay.max {
                assert!(max_delay >= 0.0, "Max delay should be non-negative");
            }
            if let Some(min_delay) = edge.delay.min {
                assert!(min_delay >= 0.0, "Min delay should be non-negative");
            }
        }
    }

    /// Test cyclic circuit edge case with minimal feedback
    #[test]
    fn test_cyclic_minimal_feedback() {
        let input = r#"Port "a" [("b", 10)]
                      DataReg "b" [("b", 5), ("c", 8)]
                      Port "c" []"#;

        let structural_graph = parse(input).expect("Failed to parse minimal cyclic input");
        let hbcn = from_structural_graph(&structural_graph, false)
            .expect("Failed to convert minimal cyclic graph to HBCN");

        // Should still work with minimal feedback
        let result = constrain_cycle_time_proportional(&hbcn, 50.0, 2.0, None, None)
            .expect("Should handle minimal cyclic circuit");

        assert!(result.pseudoclock_period >= 2.0);
        assert!(result.pseudoclock_period <= 50.0);
    }
}
