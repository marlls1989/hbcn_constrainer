use lazy_static::*;
use regex::Regex;
use std::io::{self, Write};

use crate::constrain::hbcn::is_rise;
use crate::hbcn::{CircuitNode, SolvedHBCN, Transition};

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

/// The SDC `-through` qualifier for a transition endpoint. A `Data` (`+`) transition is a rise
/// at its register/port, a `Spacer` (`-`) transition a fall.
fn through_keyword(t: &Transition) -> &'static str {
    if is_rise(t) {
        "rise_through"
    } else {
        "fall_through"
    }
}

/// Write SDC timing constraints to a writer.
///
/// Iterates the solved HBCN and emits a `set_min_delay`/`set_max_delay` per place. Each place
/// is a timing arc between two transitions — the physical start and end points — and each
/// `-through` clause is qualified by its own endpoint's transition direction (`Data` ⇒
/// `-rise_through`, `Spacer` ⇒ `-fall_through`). So the data/spacer propagation places are
/// rise→rise / fall→fall (positive unate) and the acknowledge places rise→fall / fall→rise
/// (negative unate).
///
/// # Arguments
///
/// * `writer` - Output writer for the SDC file
/// * `hbcn` - The solved HBCN whose edges are the per-place timing arcs
/// * `pseudoclock_period` - Reference period: a `max` delay within 0.1% of it (a path left at
///   the floor) adds no real constraint and is not emitted; negligible (`<= 0.001`) `min`
///   delays are likewise dropped
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
/// use hbcn::hbcn::SolvedHBCN;
/// use std::io::BufWriter;
/// use std::fs::File;
/// # let hbcn = SolvedHBCN::default();
///
/// let mut writer = BufWriter::new(File::create("constraints.sdc")?);
/// write_path_constraints(&mut writer, &hbcn, 10.0)?;
/// # Ok(())
/// # }
/// ```
pub fn write_path_constraints(
    writer: &mut dyn Write,
    hbcn: &SolvedHBCN,
    pseudoclock_period: f64,
) -> io::Result<()> {
    for ie in hbcn.edge_indices() {
        let Some((is, id)) = hbcn.edge_endpoints(ie) else {
            continue;
        };
        let src_t: &Transition = &hbcn[is].transition;
        let dst_t: &Transition = &hbcn[id].transition;
        let src = AsRef::<CircuitNode>::as_ref(&hbcn[is]);
        let dst = AsRef::<CircuitNode>::as_ref(&hbcn[id]);
        let delay = &hbcn[ie].delay;

        // Negligible min delays are not worth an SDC constraint.
        if let Some(min) = delay.min
            && min > 0.001
        {
            writeln!(
                writer,
                "set_min_delay {:.3} \\\n\t-{} {} \\\n\t-{} {}",
                min,
                through_keyword(src_t),
                src_rails(src),
                through_keyword(dst_t),
                dst_rails(dst),
            )?;
        }

        // A max delay at (within 0.1% of) the floor period adds no real constraint.
        if (delay.max - pseudoclock_period).abs() > pseudoclock_period * 1e-3 {
            writeln!(
                writer,
                "set_max_delay {:.3} \\\n\t-{} {} \\\n\t-{} {}",
                delay.max,
                through_keyword(src_t),
                src_rails(src),
                through_keyword(dst_t),
                dst_rails(dst),
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hbcn::{DelayPair, DelayedPlace, Place, TransitionEvent};
    use std::io::Cursor;
    use string_cache::DefaultAtom;

    /// Build a minimal solved HBCN: one fresh node per endpoint, one edge per place.
    /// (The SDC writer reads each edge's own endpoints, so nodes need not be shared.)
    fn solved_with(edges: Vec<(Transition, Transition, DelayPair)>) -> SolvedHBCN {
        let mut g = SolvedHBCN::default();
        for (src, dst, delay) in edges {
            let s = g.add_node(TransitionEvent {
                time: 0.0,
                transition: src,
            });
            let d = g.add_node(TransitionEvent {
                time: 0.0,
                transition: dst,
            });
            g.add_edge(
                s,
                d,
                DelayedPlace {
                    place: Place {
                        token: false,
                        is_internal: false,
                    },
                    delay,
                    slack: None,
                },
            );
        }
        g
    }

    fn port(name: &str) -> CircuitNode {
        CircuitNode::Port(DefaultAtom::from(name))
    }

    fn register(name: &str) -> CircuitNode {
        CircuitNode::Register(DefaultAtom::from(name))
    }

    /// A data (rise) transition at a node.
    fn data(node: CircuitNode) -> Transition {
        Transition::Data(node)
    }

    /// A spacer (fall) transition at a node.
    fn spacer(node: CircuitNode) -> Transition {
        Transition::Spacer(node)
    }

    /// Test SDC generation for simple port-to-port constraints
    #[test]
    fn test_sdc_port_to_port_constraints() {
        let hbcn = solved_with(vec![(
            data(port("input")),
            data(port("output")),
            DelayPair {
                min: Some(2.5),
                max: 10.0,
            },
        )]);

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &hbcn, 0.0).expect("Should write SDC");

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
        let hbcn = solved_with(vec![(
            data(port("clk")),
            data(register("reg1")),
            DelayPair {
                min: None,
                max: 5.25,
            },
        )]);

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &hbcn, 0.0).expect("Should write SDC");

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
        let hbcn = solved_with(vec![
            (
                data(port("in1")),
                data(port("out1")),
                DelayPair {
                    min: Some(1.0),
                    max: 5.0,
                },
            ),
            (
                data(port("clk")),
                data(register("counter")),
                DelayPair {
                    min: None,
                    max: 8.75,
                },
            ),
        ]);

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &hbcn, 0.0).expect("Should write multiple SDC");

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
        let hbcn = solved_with(vec![(
            data(port("precise")),
            data(port("timing")),
            DelayPair {
                // 4th decimals (4 / 6) round unambiguously down / up regardless of
                // half-to-even behaviour.
                min: Some(1.2344),
                max: 9.8766,
            },
        )]);

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &hbcn, 0.0).expect("Should write SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        assert!(sdc_content.contains("set_min_delay 1.234"));
        assert!(sdc_content.contains("set_max_delay 9.877"));
    }

    /// A min-only constraint (max == pseudoclock period) emits only `set_min_delay`,
    /// and an empty constraint set produces no output.
    #[test]
    fn test_sdc_min_only_and_empty() {
        let hbcn = solved_with(vec![(
            data(port("input")),
            data(port("output")),
            DelayPair {
                min: Some(1.5),
                max: 0.0,
            },
        )]);

        let mut output = Cursor::new(Vec::new());
        // pseudoclock_period == 0.0 suppresses the max delay (which equals it).
        write_path_constraints(&mut output, &hbcn, 0.0).expect("Should write SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");
        assert!(sdc_content.contains("set_min_delay 1.500"));
        assert!(!sdc_content.contains("set_max_delay"));

        let empty = SolvedHBCN::default();
        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &empty, 0.0).expect("Should write empty SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");
        assert!(sdc_content.is_empty());
    }

    /// Emitted constraints use TCL line continuations and `-through` clauses.
    #[test]
    fn test_sdc_tcl_structure() {
        let hbcn = solved_with(vec![(
            data(port("src")),
            data(port("dst")),
            DelayPair {
                min: Some(2.0),
                max: 8.0,
            },
        )]);

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &hbcn, 0.0).expect("Should write SDC");
        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        assert!(sdc_content.contains('\\'));
        assert!(sdc_content.contains("_through"));
        let lines: Vec<&str> = sdc_content.lines().collect();
        assert!(lines.iter().any(|l| l.starts_with("set_min_delay")));
        assert!(lines.iter().any(|l| l.starts_with("set_max_delay")));
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("\t-rise_through") || l.starts_with("\t-fall_through"))
        );
    }

    /// Per-place rise/fall: a positive-unate propagation place qualifies both `-through`
    /// clauses the same way; a negative-unate acknowledge place qualifies them oppositely.
    #[test]
    fn test_sdc_per_place_rise_fall() {
        let a = || port("a");
        let b = || port("b");
        // One channel A<->B, four places with distinct delays.
        let hbcn = solved_with(vec![
            // forward-data: rise -> rise
            (
                data(a()),
                data(b()),
                DelayPair {
                    min: None,
                    max: 11.0,
                },
            ),
            // forward-spacer: fall -> fall
            (
                spacer(a()),
                spacer(b()),
                DelayPair {
                    min: None,
                    max: 12.0,
                },
            ),
            // data-ack: rise -> fall
            (
                data(b()),
                spacer(a()),
                DelayPair {
                    min: None,
                    max: 13.0,
                },
            ),
            // spacer-ack: fall -> rise
            (
                spacer(b()),
                data(a()),
                DelayPair {
                    min: None,
                    max: 14.0,
                },
            ),
        ]);

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &hbcn, 0.0).expect("Should write SDC");
        let sdc = String::from_utf8(output.into_inner()).expect("valid UTF-8");

        // Each statement is three lines: `set_max_delay <v> \`, then two `\t-<dir>_through ...`
        // lines. Capture the qualifier on each clause, keyed by the delay value.
        let lines: Vec<&str> = sdc.lines().collect();
        let qual = |line: &str| {
            if line.contains("rise_through") {
                'r'
            } else if line.contains("fall_through") {
                'f'
            } else {
                '?'
            }
        };
        let mut found: std::collections::HashMap<String, (char, char)> = Default::default();
        for (i, l) in lines.iter().enumerate() {
            if let Some(rest) = l.strip_prefix("set_max_delay ") {
                let val = rest.split_whitespace().next().unwrap().to_string();
                found.insert(val, (qual(lines[i + 1]), qual(lines[i + 2])));
            }
        }

        assert_eq!(
            found.get("11.000"),
            Some(&('r', 'r')),
            "forward-data rise->rise"
        );
        assert_eq!(
            found.get("12.000"),
            Some(&('f', 'f')),
            "forward-spacer fall->fall"
        );
        assert_eq!(
            found.get("13.000"),
            Some(&('r', 'f')),
            "data-ack rise->fall"
        );
        assert_eq!(
            found.get("14.000"),
            Some(&('f', 'r')),
            "spacer-ack fall->rise"
        );
    }
}
