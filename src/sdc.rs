use super::{hbcn::PathConstraints, structural_graph::CircuitNode};
use itertools::Itertools;
use lazy_static::*;
use rayon::prelude::*;
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
                port_wildcard(name)
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

fn src_list<T>(sources: T) -> String
where
    T: IntoIterator<Item = CircuitNode>,
{
    let source_list: String = sources
        .into_iter()
        .map(|src| src_rails(&src))
        .intersperse(" \\\n\t".into())
        .collect();
    format!("[concat \\\n\t{} \\\n]", source_list)
}

pub fn write_path_constraints(writer: &mut dyn Write, paths: &PathConstraints) -> io::Result<()> {
    let mut paths: Vec<(String, CircuitNode, CircuitNode)> = paths
        .iter()
        .map(|((src, dst), val)| (format!("{:.3}", val), dst.clone(), src.clone()))
        .collect();
    paths.par_sort_unstable();

    for ((val, dst), sources) in paths
        .into_iter()
        .group_by(|(val, dst, _src)| (val.clone(), dst.clone()))
        .into_iter()
    {
        let sources = sources.map(|(_val, _dst, src)| src);
        writeln!(
            writer,
            "set_max_delay {} -through {} -through {} -from [get_clock clk]",
            val,
            src_list(sources),
            dst_rails(&dst),
        )?;
    }

    Ok(())
}

pub fn write_path_quantised_constraints(
    writer: &mut dyn Write,
    paths: &PathConstraints,
) -> io::Result<()> {
    let mut paths: Vec<(String, CircuitNode, CircuitNode)> = paths
        .iter()
        .map(|((src, dst), val)| (format!("{:.0}", val), dst.clone(), src.clone()))
        .collect();
    paths.par_sort_unstable();

    for ((val, dst), sources) in paths
        .into_iter()
        .group_by(|(val, dst, _src)| (val.clone(), dst.clone()))
        .into_iter()
    {
        let sources = sources.map(|(_val, _dst, src)| src);
        writeln!(
            writer,
            "set_multicycle_path {} -through {} -through {} -from [get_clock clk]",
            val,
            src_list(sources),
            dst_rails(&dst),
        )?;
    }

    Ok(())
}
