use super::{hbcn::PathConstraints, structural_graph::CircuitNode};
use lazy_static::*;
use regex::Regex;
use std::io::{self, Write};

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

fn port_instance(s: &str) -> String {
    lazy_static! {
        static ref REPLACE_RE: Regex = Regex::new(r"^port:([^/]+)/(.+)").unwrap();
        static ref INDEX_RE: Regex = Regex::new(r"^(.+)\[([0-9]+)\]").unwrap();
    }

    let s = if let Some(c) = REPLACE_RE.captures(s) {
        format!("inst:{}/i{}", &c[1], &c[2])
    } else {
        format!("inst:{}", s)
    };

    if let Some(c) = INDEX_RE.captures(&s) {
        format!("{}_{}", &c[1], &c[2])
    } else {
        s
    }
}

fn dst_rails(s: &CircuitNode) -> String {
    match s {
        CircuitNode::Port(name) => {
            format!(
                "[list [get_ports [vfind {{{}}}] -filter {{direction == out}}] [get_pins -of_objects [get_cells [vfind {{{}/*}}]] -filter {{direction == in}}]]",
                port_wildcard(name),
                port_instance(name),
            )
        }
        CircuitNode::Register { name, .. } => format!(
            "[get_pins -of_objects [get_cells [vfind {{{}/*}}] -filter {{is_sequential == true}}] -filter {{direction == in}}]",
            name
        ),
    }
}

fn src_rails(s: &CircuitNode) -> String {
    match s {
        CircuitNode::Port(name) => {
            format!(
                "[get_ports [vfind {{{}}}] -filter {{direction == in}}]",
                port_wildcard(name)
            )
        }
        CircuitNode::Register { name, .. } => format!(
            "[get_pins -of_objects [get_cells [vfind {{{}/*}}] -filter {{is_sequential == true}}] -filter {{direction == out}}]",
            name
        ),
    }
}

pub fn write_path_constraints(writer: &mut dyn Write, paths: &PathConstraints) -> io::Result<()> {
    for ((src, dst), val) in paths.iter() {
        if let Some(val) = val.min {
            writeln!(
                writer,
                "set_min_delay {:.3} \\\n\t-through {} \\\n\t-through {}",
                val,
                src_rails(src),
                dst_rails(dst),
            )?;
        }

        if let Some(val) = val.max {
            writeln!(
                writer,
                "set_max_delay {:.3} \\\n\t-through {} \\\n\t-through {}",
                val,
                src_rails(src),
                dst_rails(dst),
            )?;
        }
    }

    Ok(())
}
