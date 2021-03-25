use super::{hbcn::PathConstraints, structural_graph::CircuitNode};
use itertools::*;
use lazy_static::*;
use rayon::prelude::*;
use regex::Regex;

fn port_wildcard(s: &str) -> String {
    lazy_static! {
        static ref INDEX_RE: Regex = Regex::new(r"^(.+)(\[[0-9]+\])").unwrap();
    }

    if let Some(c) = INDEX_RE.captures(s) {
        format!("{}_*{}", &c[1], &c[2])
    } else {
        format!("{}_*", s)
    }
}

fn dst_rails(s: &CircuitNode) -> String {
    match s {
        CircuitNode::Port(name) => {
            format!(
                "[get_db [vfind {{{}}}] -if {{.direction == out}}]",
                port_wildcard(&name)
            )
        }
        CircuitNode::Register { name, .. } => format!(
            "[get_pin -of_objects [vfind {{{}/*}}] -filter {{(is_data == true) && (direction == in)}}]",
            name
        ),
    }
}

fn src_rails(s: &CircuitNode) -> String {
    match s {
        CircuitNode::Port(name) => {
            format!(
                "[get_db [vfind {{{}}}] -if {{.direction == in}}]",
                port_wildcard(&name)
            )
        }
        CircuitNode::Register { name, .. } => format!(
            "[get_pin -of_objects [vfind {{{}/*}}] -filter {{is_clock_pin == true}}]",
            name
        ),
    }
}

pub fn write_path_constraints(paths: &PathConstraints) -> String {
    paths
        .par_iter()
        .map(|((src, dst), val)| {
            format!(
                "set_multicycle_path -from {} -to {} {}\n",
                src_rails(&src),
                dst_rails(&dst),
                val
            )
        })
        .reduce(|| "".to_owned(), |a, b| [a, b].concat())
}
