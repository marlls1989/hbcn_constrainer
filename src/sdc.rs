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

fn dst_rails(s: &CircuitNode) -> String {
    match s {
        CircuitNode::Port(name) => {
            format!(
                "[get_ports [vfind {{{}}}] -filter {{direction == out}}]",
                port_wildcard(&name)
            )
        }
        CircuitNode::Register { name, .. } => format!(
            //"[get_pin -of_objects [vfind {{{}/*}}] -filter {{is_clock_pin == true}}]",
            "[get_pins -of_objects [vfind {{{}/*}}] -filter {{(is_data == true) && (direction == in)}}]",
            name
        ),
    }
}

fn src_rails(s: &CircuitNode) -> String {
    match s {
        CircuitNode::Port(name) => {
            format!(
                "[get_ports [vfind {{{}}}] -filter {{direction == in}}]",
                port_wildcard(&name)
            )
        }
        CircuitNode::Register { name, .. } => format!(
            "[get_pins -of_objects [vfind {{{}/*}}] -filter {{direction == out}}]",
            name
        ),
    }
}

pub fn write_path_constraints(writer: &mut dyn Write, paths: &PathConstraints) -> io::Result<()> {
    for ((src, dst), val) in paths.iter() {
        writeln!(
            writer,
            "set_multicycle_path {} -through {} -through {} -from [get_clock clk]",
            val,
            src_rails(&src),
            dst_rails(&dst),
        )?;
    }

    Ok(())
}
