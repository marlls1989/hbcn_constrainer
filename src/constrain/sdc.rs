use crate::constrain::hbcn::PathConstraints;
use lazy_static::*;
use regex::Regex;
use std::io::{self, Write};

use crate::hbcn::CircuitNode;

fn port_wildcard(s: &str) -> String {
    lazy_static! {
        static ref INDEX_RE: Regex = Regex::new(r"^(.+)(\[[0-9]+\])").unwrap();
    }

    if let Some(c) = INDEX_RE.captures(s) {
        format!("{}_*{} {}_ack", &c[1], &c[2], &c[1])
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
        CircuitNode::Register(name) => format!(
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
                port_wildcard(name),
            )
        }
        CircuitNode::Register(name) => format!(
            "[get_pins -of_objects [get_cells [vfind {{{}/*}}] -filter {{is_sequential == true}}] -filter {{direction == out}}]",
            name
        ),
    }
}

/// Write SDC timing constraints to a writer.
///
/// This function generates SDC format constraints for all paths in the path constraints
/// map. It creates `set_min_delay` and `set_max_delay` commands with proper `-through`
/// clauses that reference the source and destination circuit nodes.
///
/// # Arguments
///
/// * `writer` - Output writer for the SDC file
/// * `paths` - Map of (source, destination) pairs to delay constraints
/// * `pseudoclock_period` - The pseudo-clock period (used to filter max delays equal to the clock)
///
/// # Returns
///
/// Returns `Ok(())` on success, or an I/O error if writing fails.
///
/// # Example
///
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use hbcn::constrain::sdc::write_path_constraints;
/// use std::collections::HashMap;
/// use std::io::BufWriter;
/// use std::fs::File;
/// use hbcn::hbcn::{DelayPair, CircuitNode};
/// # let path_constraints: std::collections::HashMap<(CircuitNode, CircuitNode), DelayPair> = HashMap::new();
///
/// let mut writer = BufWriter::new(File::create("constraints.sdc")?);
/// write_path_constraints(&mut writer, &path_constraints, 10.0)?;
/// # Ok(())
/// # }
/// ```
pub fn write_path_constraints(
    writer: &mut dyn Write,
    paths: &PathConstraints,
    pseudoclock_period: f64,
) -> io::Result<()> {
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

        if val.max != pseudoclock_period {
            writeln!(
                writer,
                "set_max_delay {:.3} \\\n\t-through {} \\\n\t-through {}",
                val.max,
                src_rails(src),
                dst_rails(dst),
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hbcn::CircuitNode;
    use crate::hbcn::DelayPair;
    use std::collections::HashMap;
    use std::io::Cursor;
    use string_cache::DefaultAtom;

    /// Test SDC generation for simple port-to-port constraints
    #[test]
    fn test_sdc_port_to_port_constraints() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port(DefaultAtom::from("input")),
                CircuitNode::Port(DefaultAtom::from("output")),
            ),
            DelayPair {
                min: Some(2.5),
                max: 10.0,
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints, 0.0).expect("Should write SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should contain both min and max delay constraints
        assert!(sdc_content.contains("set_min_delay 2.500"));
        assert!(sdc_content.contains("set_max_delay 10.000"));
        assert!(sdc_content.contains("input_*"));
        assert!(sdc_content.contains("output_*"));
    }

    /// Test SDC generation for register constraints
    #[test]
    fn test_sdc_register_constraints() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port(DefaultAtom::from("clk")),
                CircuitNode::Register(DefaultAtom::from("reg1")),
            ),
            DelayPair {
                min: None,
                max: 5.25,
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints, 0.0).expect("Should write SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should only contain max delay (no min specified)
        assert!(sdc_content.contains("set_max_delay 5.250"));
        assert!(!sdc_content.contains("set_min_delay"));
        assert!(sdc_content.contains("clk_*"));
        assert!(sdc_content.contains("reg1/*"));
        assert!(sdc_content.contains("is_sequential == true"));
    }

    /// Test port wildcard generation
    #[test]
    fn test_port_wildcard_generation() {
        // Simple port name
        assert_eq!(port_wildcard("clk"), "clk_*");

        // Port with index
        assert_eq!(port_wildcard("data[0]"), "data_*[0] data_ack");
        assert_eq!(port_wildcard("bus[15]"), "bus_*[15] bus_ack");

        // Port without index
        assert_eq!(port_wildcard("reset"), "reset_*");
    }

    /// Test port instance generation
    #[test]
    fn test_port_instance_generation() {
        // Simple instance
        assert_eq!(port_instance("simple"), "inst:simple");

        // Instance with index
        assert_eq!(port_instance("indexed[5]"), "inst:indexed_5");

        // Port with path
        assert_eq!(port_instance("port:module/signal"), "inst:module/isignal");

        // Complex case
        assert_eq!(port_instance("port:cpu/data[8]"), "inst:cpu/idata_8");
    }

    /// Test multiple constraints
    #[test]
    fn test_sdc_multiple_constraints() {
        let mut constraints = HashMap::new();

        // Port to port
        constraints.insert(
            (
                CircuitNode::Port(DefaultAtom::from("in1")),
                CircuitNode::Port(DefaultAtom::from("out1")),
            ),
            DelayPair {
                min: Some(1.0),
                max: 5.0,
            },
        );

        // Port to register
        constraints.insert(
            (
                CircuitNode::Port(DefaultAtom::from("clk")),
                CircuitNode::Register(DefaultAtom::from("counter")),
            ),
            DelayPair {
                min: None,
                max: 8.75,
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints, 0.0).expect("Should write multiple SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should contain all constraints
        assert!(sdc_content.contains("set_min_delay 1.000"));
        assert!(sdc_content.contains("set_max_delay 5.000"));
        assert!(sdc_content.contains("set_max_delay 8.750"));

        // Should contain proper node references
        assert!(sdc_content.contains("in1_*"));
        assert!(sdc_content.contains("out1_*"));
        assert!(sdc_content.contains("clk_*"));
        assert!(sdc_content.contains("counter/*"));
    }

    /// src_rails / dst_rails produce the expected Genus directives per node type.
    #[test]
    fn test_src_dst_rails_generation() {
        let port = CircuitNode::Port(DefaultAtom::from("input_port"));
        let src = src_rails(&port);
        assert!(src.contains("get_ports"));
        assert!(src.contains("input_port_*"));
        assert!(src.contains("direction == in"));

        let dst = dst_rails(&port);
        assert!(dst.contains("get_ports"));
        assert!(dst.contains("input_port_*"));
        assert!(dst.contains("direction == out"));
        assert!(dst.contains("get_pins"));

        let reg = CircuitNode::Register(DefaultAtom::from("test_reg"));
        let src = src_rails(&reg);
        assert!(src.contains("get_pins"));
        assert!(src.contains("test_reg/*"));
        assert!(src.contains("is_sequential == true"));
        assert!(src.contains("direction == out"));

        let dst = dst_rails(&reg);
        assert!(dst.contains("test_reg/*"));
        assert!(dst.contains("is_sequential == true"));
        assert!(dst.contains("direction == in"));
    }

    /// Delays are formatted to three decimal places (rounded).
    #[test]
    fn test_sdc_decimal_precision() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port(DefaultAtom::from("precise")),
                CircuitNode::Port(DefaultAtom::from("timing")),
            ),
            DelayPair {
                // 4th decimals (4 / 6) round unambiguously down / up regardless of
                // half-to-even behaviour.
                min: Some(1.2344),
                max: 9.8766,
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints, 0.0).expect("Should write SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        assert!(sdc_content.contains("set_min_delay 1.234"));
        assert!(sdc_content.contains("set_max_delay 9.877"));
    }

    /// A min-only constraint (max == pseudoclock period) emits only `set_min_delay`,
    /// and an empty constraint set produces no output.
    #[test]
    fn test_sdc_min_only_and_empty() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port(DefaultAtom::from("input")),
                CircuitNode::Port(DefaultAtom::from("output")),
            ),
            DelayPair {
                min: Some(1.5),
                max: 0.0,
            },
        );

        let mut output = Cursor::new(Vec::new());
        // pseudoclock_period == 0.0 suppresses the max delay (which equals it).
        write_path_constraints(&mut output, &constraints, 0.0).expect("Should write SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");
        assert!(sdc_content.contains("set_min_delay 1.500"));
        assert!(!sdc_content.contains("set_max_delay"));

        let empty = PathConstraints::new();
        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &empty, 0.0).expect("Should write empty SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");
        assert!(sdc_content.is_empty());
    }

    /// Emitted constraints use TCL line continuations and `-through` clauses.
    #[test]
    fn test_sdc_tcl_structure() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port(DefaultAtom::from("src")),
                CircuitNode::Port(DefaultAtom::from("dst")),
            ),
            DelayPair {
                min: Some(2.0),
                max: 8.0,
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints, 0.0).expect("Should write SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        assert!(sdc_content.contains('\\'));
        assert!(sdc_content.contains("-through"));
        let lines: Vec<&str> = sdc_content.lines().collect();
        assert!(lines.iter().any(|l| l.starts_with("set_min_delay")));
        assert!(lines.iter().any(|l| l.starts_with("set_max_delay")));
        assert!(lines.iter().any(|l| l.starts_with("\t-through")));
    }
}
