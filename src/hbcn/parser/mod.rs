mod ast;

// Include the generated parser with clippy warnings suppressed
#[allow(clippy::all)]
mod parser {
    #![allow(clippy::all)]
    #![allow(dead_code)]
    #![allow(unused_variables)]
    #![allow(unused_imports)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    #![allow(non_upper_case_globals)]
    include!(concat!(env!("OUT_DIR"), "/hbcn/parser/parser.rs"));
}

use crate::hbcn::{CircuitNode, DelayedPlace, HBCN, Place, Transition, validate_hbcn};
use anyhow::Result;
use std::collections::{HashMap, HashSet};

/// Parse an HBCN from the grammar format.
///
/// This function parses the HBCN format defined in the grammar and generates a
/// `HBCN<Transition, DelayedPlace>`. Node names are used to determine if a circuit node
/// is a register or a port: all names starting with "port:" are ports, all others are registers.
///
/// After parsing, the HBCN is validated using `validate_hbcn`.
///
/// # Arguments
///
/// * `input` - The input string to parse
///
/// # Returns
///
/// Returns `Ok(HBCN<Transition, DelayedPlace>)` if parsing and validation succeed,
/// or an `Error` if parsing fails or validation fails.
///
/// # Example
///
/// ```
/// use hbcn::hbcn::parser::parse_hbcn;
///
/// let input = r#"
///     * +{port:in} => +{reg1} : (1.0, 2.0)
///     +{reg1} => -{port:in} : (0.5, 1.5)
///     -{port:in} => -{reg1} : (0.5, 1.0)
///     -{reg1} => +{port:in} : (0.0, 1.0)
/// "#;
///
/// let hbcn = parse_hbcn(input).unwrap();
/// assert!(hbcn.node_count() > 0);
/// ```
pub fn parse_hbcn(input: &str) -> Result<HBCN<Transition, DelayedPlace>> {
    let adjacency_list = parser::AdjacencyListParser::new()
        .parse(input)
        .map_err(|e| anyhow::anyhow!("Failed to parse HBCN input: {}", e))?;

    // Collect all unique transitions
    let unique_transitions: HashSet<_> = adjacency_list
        .iter()
        .flat_map(|entry| [&entry.source, &entry.target])
        .collect();

    // Build graph nodes and create transition map
    let (hbcn, transition_map) = unique_transitions.into_iter().fold(
        (HBCN::new(), HashMap::new()),
        |(mut graph, mut map), ast_trans| {
            let circuit_node = match ast_trans {
                ast::Transition::Data(sym) | ast::Transition::Spacer(sym) => {
                    if sym.as_ref().starts_with("port:") {
                        CircuitNode::Port(sym.clone())
                    } else {
                        CircuitNode::Register(sym.clone())
                    }
                }
            };

            let hbcn_transition = match ast_trans {
                ast::Transition::Data(_) => Transition::Data(circuit_node),
                ast::Transition::Spacer(_) => Transition::Spacer(circuit_node),
            };

            let node_idx = graph.add_node(hbcn_transition);
            map.insert(ast_trans.clone(), node_idx);
            (graph, map)
        },
    );

    // Add edges
    let hbcn = adjacency_list.iter().fold(hbcn, |mut graph, entry| {
        let source_idx = transition_map[&entry.source];
        let target_idx = transition_map[&entry.target];

        graph.add_edge(
            source_idx,
            target_idx,
            DelayedPlace {
                place: Place {
                    token: entry.token,
                    is_internal: false,
                },
                delay: entry.delay.clone(),
                slack: None,
            },
        );
        graph
    });

    // Validate the HBCN
    validate_hbcn(&hbcn).map_err(|e| anyhow::anyhow!("HBCN validation failed: {}", e))?;

    Ok(hbcn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hbcn::serialisation::serialize_hbcn_transition;
    use crate::hbcn::test_helpers::create_valid_two_channel_hbcn;
    use crate::hbcn::{DelayPair, MarkablePlace, Named, Transition, validate_hbcn};

    #[test]
    fn test_parse_hbcn_basic() {
        // Test parsing a basic HBCN with ports and registers
        // For a valid channel from a to b, we need 4 places:
        // Data(a) -> Data(b), Data(b) -> Spacer(a), Spacer(a) -> Spacer(b), Spacer(b) -> Data(a)
        // And exactly one token

        // Channel from port:a to reg1
        let input = r#"
            * +{port:a} => +{reg1} : (1.0, 2.0)
            +{reg1} => -{port:a} : (0.5, 1.5)
            -{port:a} => -{reg1} : (0.5, 1.0)
            -{reg1} => +{port:a} : (0.0, 1.0)
        "#;
        let result = parse_hbcn(input);
        if let Err(e) = &result {
            eprintln!("Parse error: {}", e);
        }
        assert!(result.is_ok(), "Should parse basic HBCN");
    }

    #[test]
    fn test_parse_hbcn_with_tokens() {
        // Test parsing HBCN with token markers
        // Channel from a to b with token on Data(a) -> Data(b)
        let input = r#"
            * +{a} => +{b} : (1.0, 2.0)
            +{b} => -{a} : (0.5, 1.5)
            -{a} => -{b} : (0.5, 1.0)
            -{b} => +{a} : (0.0, 1.0)
        "#;
        let result = parse_hbcn(input);
        if let Err(e) = &result {
            eprintln!("Parse error: {}", e);
        }
        assert!(result.is_ok(), "Should parse HBCN with tokens");
        let hbcn = result.unwrap();
        // Check that token edge was marked
        let mut found_token = false;
        for edge_idx in hbcn.edge_indices() {
            let place = &hbcn[edge_idx];
            if place.is_marked() {
                found_token = true;
                break;
            }
        }
        assert!(found_token, "Should find at least one token");
    }

    #[test]
    fn parse_empty_adjacency_list() {
        let input = "";
        let result = super::parser::AdjacencyListParser::new().parse(input);
        assert!(result.is_ok());
        let list = result.unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn parse_single_edge_without_token() {
        let input = r#"+{a} => -{b} : (1,2)"#;
        let list = super::parser::AdjacencyListParser::new()
            .parse(input)
            .expect("should parse single edge");
        assert_eq!(list.len(), 1);
        let e = &list[0];

        match &e.source {
            ast::Transition::Data(sym) => assert_eq!(sym.as_ref(), "a"),
            _ => panic!("expected Data transition for source"),
        }
        match &e.target {
            ast::Transition::Spacer(sym) => assert_eq!(sym.as_ref(), "b"),
            _ => panic!("expected Spacer transition for target"),
        }
        assert_eq!(
            e.delay,
            DelayPair {
                min: Some(1.0),
                max: 2.0
            }
        );
        assert!(!e.token);
    }

    #[test]
    fn parse_single_edge_with_token() {
        let input = r#"* -{x} => +{y} : 3.5"#;
        let list = super::parser::AdjacencyListParser::new()
            .parse(input)
            .expect("should parse token edge");
        assert_eq!(list.len(), 1);
        let e = &list[0];

        match &e.source {
            ast::Transition::Spacer(sym) => assert_eq!(sym.as_ref(), "x"),
            _ => panic!("expected Spacer source"),
        }
        match &e.target {
            ast::Transition::Data(sym) => assert_eq!(sym.as_ref(), "y"),
            _ => panic!("expected Data target"),
        }
        assert_eq!(
            e.delay,
            DelayPair {
                min: None,
                max: 3.5
            }
        );
        assert!(e.token);
    }

    #[test]
    fn parse_multiple_edges_and_delay_variants() {
        let input = r#"
            +{n1} => +{n2} : (0.5,0.0)
            -{n2} => -{n3} : 0.0 
            * +{n3} => -{n1} : (2,4.25)
        "#;
        let list = super::parser::AdjacencyListParser::new()
            .parse(input)
            .expect("should parse multiple edges");
        assert_eq!(list.len(), 3);

        assert_eq!(
            list[0].delay,
            DelayPair {
                min: Some(0.5),
                max: 0.0
            }
        );
        assert_eq!(
            list[1].delay,
            DelayPair {
                min: None,
                max: 0.0
            }
        );
        assert_eq!(
            list[2].delay,
            DelayPair {
                min: Some(2.0),
                max: 4.25
            }
        );
        assert!(list[2].token);

        let get_name = |t: &ast::Transition| match t {
            ast::Transition::Data(s) | ast::Transition::Spacer(s) => s.as_ref().to_string(),
        };
        assert_eq!(get_name(&list[0].source), "n1");
        assert_eq!(get_name(&list[0].target), "n2");
        assert_eq!(get_name(&list[1].source), "n2");
        assert_eq!(get_name(&list[1].target), "n3");
        assert_eq!(get_name(&list[2].source), "n3");
        assert_eq!(get_name(&list[2].target), "n1");
    }

    #[test]
    fn parse_floating_and_integer_numbers() {
        let input = r#"
            +{a} => +{b} : (10,20.75)
            -{b} => +{c} : (1.25,2)
        "#;
        let list = super::parser::AdjacencyListParser::new()
            .parse(input)
            .expect("should parse mixed numbers");
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0].delay,
            DelayPair {
                min: Some(10.0),
                max: 20.75
            }
        );
        assert_eq!(
            list[1].delay,
            DelayPair {
                min: Some(1.25),
                max: 2.0
            }
        );

        assert!(!list[0].token);
        assert!(!list[1].token);
    }

    fn edge_tuple_from_ast(
        e: &super::ast::AdjacencyEntry,
    ) -> (char, String, char, String, Option<f64>, f64, bool) {
        let (sk, sn) = match &e.source {
            super::ast::Transition::Data(s) => ('+', s.as_ref().to_string()),
            super::ast::Transition::Spacer(s) => ('-', s.as_ref().to_string()),
        };
        let (tk, tn) = match &e.target {
            super::ast::Transition::Data(s) => ('+', s.as_ref().to_string()),
            super::ast::Transition::Spacer(s) => ('-', s.as_ref().to_string()),
        };
        (sk, sn, tk, tn, e.delay.min, e.delay.max, e.token)
    }

    #[test]
    fn parse_node_with_escaped_braces() {
        // Test parsing nodes with TCL-style escaped braces
        let input = r#"+{node\{with\}} => -{other\{name\}} : (1,2)"#;
        let list = super::parser::AdjacencyListParser::new()
            .parse(input)
            .expect("should parse node with escaped braces");
        assert_eq!(list.len(), 1);
        let e = &list[0];

        match &e.source {
            ast::Transition::Data(sym) => assert_eq!(sym.as_ref(), "node{with}"),
            _ => panic!("expected Data transition for source"),
        }
        match &e.target {
            ast::Transition::Spacer(sym) => assert_eq!(sym.as_ref(), "other{name}"),
            _ => panic!("expected Spacer transition for target"),
        }
        assert_eq!(
            e.delay,
            DelayPair {
                min: Some(1.0),
                max: 2.0
            }
        );
        assert!(!e.token);
    }

    #[test]
    fn serialize_and_parse_round_trip_basic() {
        // Create a valid two-channel HBCN: a -> b -> c
        let g = create_valid_two_channel_hbcn(
            "a", "b", "c", 2.5, 1.0, // forward and backward weights for (a, b)
            0.0, 0.5, // forward and backward weights for (b, c)
            0,   // token on Data(a) -> Data(b) for channel (a, b)
            1,   // token on Data(c) -> Spacer(b) for channel (b, c)
        );

        // Validate the HBCN is valid before serialization
        validate_hbcn(&g).expect("Created HBCN should be valid");

        let text = serialize_hbcn_transition(&g);
        let parsed = super::parser::AdjacencyListParser::new()
            .parse(&text)
            .expect("parser should accept serialized output");

        assert_eq!(g.edge_count(), parsed.len());
        for (i, e_ast) in parsed.iter().enumerate() {
            let ie = g.edge_indices().nth(i).unwrap();
            let (s, t) = g.edge_endpoints(ie).unwrap();
            let e = &g[ie];
            let (sk, sn, tk, tn, min, max, token) = edge_tuple_from_ast(e_ast);

            let s_tr = &g[s];
            let t_tr = &g[t];
            let (gsk, gsn) = match s_tr {
                Transition::Data(n) => ('+', n.name().as_ref().to_string()),
                Transition::Spacer(n) => ('-', n.name().as_ref().to_string()),
            };
            let (gtk, gtn) = match t_tr {
                Transition::Data(n) => ('+', n.name().as_ref().to_string()),
                Transition::Spacer(n) => ('-', n.name().as_ref().to_string()),
            };

            assert_eq!((sk, sn, tk, tn), (gsk, gsn, gtk, gtn));
            assert_eq!((min, max, token), (e.delay.min, e.delay.max, e.is_marked()));
        }
    }

    #[test]
    fn serialize_and_parse_delay_variants() {
        // Create a valid two-channel HBCN: n1 -> n2 -> n3
        let g = create_valid_two_channel_hbcn(
            "n1", "n2", "n3", 0.0, 0.0, // forward and backward weights for (n1, n2)
            3.0, 0.0, // forward and backward weights for (n2, n3)
            2,   // token on Spacer(n1) -> Spacer(n2) for channel (n1, n2)
            0,   // token on Data(n2) -> Data(n3) for channel (n2, n3)
        );

        // Validate the HBCN is valid before serialization
        validate_hbcn(&g).expect("Created HBCN should be valid");

        let text = serialize_hbcn_transition(&g);
        let parsed = super::parser::AdjacencyListParser::new()
            .parse(&text)
            .expect("parser should accept serialized output");

        assert_eq!(g.edge_count(), parsed.len());
        for (i, e_ast) in parsed.iter().enumerate() {
            let ie = g.edge_indices().nth(i).unwrap();
            let (s, t) = g.edge_endpoints(ie).unwrap();
            let e = &g[ie];
            let (sk, sn, tk, tn, min, max, token) = edge_tuple_from_ast(e_ast);

            let s_tr = &g[s];
            let t_tr = &g[t];
            let (gsk, gsn) = match s_tr {
                Transition::Data(n) => ('+', n.name().as_ref().to_string()),
                Transition::Spacer(n) => ('-', n.name().as_ref().to_string()),
            };
            let (gtk, gtn) = match t_tr {
                Transition::Data(n) => ('+', n.name().as_ref().to_string()),
                Transition::Spacer(n) => ('-', n.name().as_ref().to_string()),
            };

            assert_eq!((sk, sn, tk, tn), (gsk, gsn, gtk, gtn));
            assert_eq!((min, max, token), (e.delay.min, e.delay.max, e.is_marked()));
        }
    }

    #[test]
    fn serialize_and_parse_round_trip_with_braces_in_name() {
        // Test that nodes with braces in their names are properly escaped/unescaped
        use crate::hbcn::{CircuitNode, DelayPair, DelayedPlace, Place, Transition};
        use petgraph::stable_graph::StableGraph;
        use string_cache::DefaultAtom;

        let mut hbcn: StableGraph<Transition, DelayedPlace> = StableGraph::new();

        // Create nodes with braces in names
        let n1 = hbcn.add_node(Transition::Data(CircuitNode::Port(DefaultAtom::from(
            "node{with}",
        ))));
        let n2 = hbcn.add_node(Transition::Spacer(CircuitNode::Register(
            DefaultAtom::from("other{name}"),
        )));

        hbcn.add_edge(
            n1,
            n2,
            DelayedPlace {
                place: Place {
                    token: false,
                    is_internal: false,
                },
                delay: DelayPair {
                    min: Some(1.0),
                    max: 2.0,
                },
                slack: None,
            },
        );

        // Serialize and parse back
        let text = serialize_hbcn_transition(&hbcn);
        let parsed = super::parser::AdjacencyListParser::new()
            .parse(&text)
            .expect("parser should accept serialized output");

        assert_eq!(parsed.len(), 1);
        let e = &parsed[0];

        match &e.source {
            ast::Transition::Data(sym) => assert_eq!(sym.as_ref(), "node{with}"),
            _ => panic!("expected Data transition"),
        }
        match &e.target {
            ast::Transition::Spacer(sym) => assert_eq!(sym.as_ref(), "other{name}"),
            _ => panic!("expected Spacer transition"),
        }
    }
}
