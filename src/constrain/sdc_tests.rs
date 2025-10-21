#[cfg(test)]
mod sdc_tests {
    use super::*;
    use crate::constrain::hbcn::DelayPair;
    use crate::structural_graph::CircuitNode;
    use std::collections::HashMap;
    use std::io::Cursor;

    /// Test SDC generation for simple port-to-port constraints
    #[test]
    fn test_sdc_port_to_port_constraints() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port("input".to_string()),
                CircuitNode::Port("output".to_string()),
            ),
            DelayPair {
                min: Some(2.5),
                max: Some(10.0),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write SDC");

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
                CircuitNode::Port("clk".to_string()),
                CircuitNode::Register {
                    name: "reg1".to_string(),
                    data_path: "data".to_string(),
                    control_path: "ctrl".to_string(),
                    output_path: "out".to_string(),
                },
            ),
            DelayPair {
                min: None,
                max: Some(5.25),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should only contain max delay (no min specified)
        assert!(sdc_content.contains("set_max_delay 5.250"));
        assert!(!sdc_content.contains("set_min_delay"));
        assert!(sdc_content.contains("clk_*"));
        assert!(sdc_content.contains("reg1/*"));
        assert!(sdc_content.contains("is_sequential == true"));
    }

    /// Test SDC generation with only min constraints
    #[test]
    fn test_sdc_min_only_constraints() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port("input".to_string()),
                CircuitNode::Port("output".to_string()),
            ),
            DelayPair {
                min: Some(1.5),
                max: None,
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should only contain min delay
        assert!(sdc_content.contains("set_min_delay 1.500"));
        assert!(!sdc_content.contains("set_max_delay"));
    }

    /// Test SDC generation with empty constraints
    #[test]
    fn test_sdc_empty_constraints() {
        let constraints = HashMap::new();

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write empty SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should be empty
        assert!(sdc_content.is_empty());
    }

    /// Test SDC generation with multiple constraints
    #[test]
    fn test_sdc_multiple_constraints() {
        let mut constraints = HashMap::new();
        
        // Port to port
        constraints.insert(
            (
                CircuitNode::Port("in1".to_string()),
                CircuitNode::Port("out1".to_string()),
            ),
            DelayPair {
                min: Some(1.0),
                max: Some(5.0),
            },
        );

        // Port to register
        constraints.insert(
            (
                CircuitNode::Port("clk".to_string()),
                CircuitNode::Register {
                    name: "counter".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: None,
                max: Some(8.75),
            },
        );

        // Register to port
        constraints.insert(
            (
                CircuitNode::Register {
                    name: "buffer".to_string(),
                    data_path: "data".to_string(),
                    control_path: "ctrl".to_string(),
                    output_path: "out".to_string(),
                },
                CircuitNode::Port("result".to_string()),
            ),
            DelayPair {
                min: Some(2.25),
                max: None,
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write multiple SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should contain all constraints
        assert!(sdc_content.contains("set_min_delay 1.000"));
        assert!(sdc_content.contains("set_max_delay 5.000"));
        assert!(sdc_content.contains("set_max_delay 8.750"));
        assert!(sdc_content.contains("set_min_delay 2.250"));

        // Should contain proper node references
        assert!(sdc_content.contains("in1_*"));
        assert!(sdc_content.contains("out1_*"));
        assert!(sdc_content.contains("clk_*"));
        assert!(sdc_content.contains("counter/*"));
        assert!(sdc_content.contains("buffer/*"));
        assert!(sdc_content.contains("result_*"));
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

    /// Test src_rails generation for different node types
    #[test]
    fn test_src_rails_generation() {
        // Port node
        let port = CircuitNode::Port("input_port".to_string());
        let src_rails = src_rails(&port);
        assert!(src_rails.contains("get_ports"));
        assert!(src_rails.contains("input_port_*"));
        assert!(src_rails.contains("direction == in"));

        // Register node
        let register = CircuitNode::Register {
            name: "test_reg".to_string(),
            data_path: "d".to_string(),
            control_path: "c".to_string(),
            output_path: "q".to_string(),
        };
        let src_rails = src_rails(&register);
        assert!(src_rails.contains("get_pins"));
        assert!(src_rails.contains("test_reg/*"));
        assert!(src_rails.contains("is_sequential == true"));
        assert!(src_rails.contains("direction == out"));
    }

    /// Test dst_rails generation for different node types
    #[test]
    fn test_dst_rails_generation() {
        // Port node
        let port = CircuitNode::Port("output_port".to_string());
        let dst_rails = dst_rails(&port);
        assert!(dst_rails.contains("get_ports"));
        assert!(dst_rails.contains("output_port_*"));
        assert!(dst_rails.contains("direction == out"));
        assert!(dst_rails.contains("get_pins"));

        // Register node
        let register = CircuitNode::Register {
            name: "dest_reg".to_string(),
            data_path: "data".to_string(),
            control_path: "ctrl".to_string(),
            output_path: "out".to_string(),
        };
        let dst_rails = dst_rails(&register);
        assert!(dst_rails.contains("get_pins"));
        assert!(dst_rails.contains("dest_reg/*"));
        assert!(dst_rails.contains("is_sequential == true"));
        assert!(dst_rails.contains("direction == in"));
    }

    /// Test SDC formatting with precise decimal places
    #[test]
    fn test_sdc_decimal_precision() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port("precise".to_string()),
                CircuitNode::Port("timing".to_string()),
            ),
            DelayPair {
                min: Some(1.2345),
                max: Some(9.8765),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should format to 3 decimal places
        assert!(sdc_content.contains("set_min_delay 1.235"));
        assert!(sdc_content.contains("set_max_delay 9.877"));
    }

    /// Test SDC constraint structure and TCL formatting
    #[test]
    fn test_sdc_tcl_structure() {
        let mut constraints = HashMap::new();
        constraints.insert(
            (
                CircuitNode::Port("src".to_string()),
                CircuitNode::Port("dst".to_string()),
            ),
            DelayPair {
                min: Some(2.0),
                max: Some(8.0),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should contain proper TCL line continuations
        assert!(sdc_content.contains("\\"));
        assert!(sdc_content.contains("-through"));
        
        // Should have proper structure for both min and max
        let lines: Vec<&str> = sdc_content.lines().collect();
        assert!(lines.len() >= 6); // At least 3 lines each for min and max constraints
        
        // Each constraint should span multiple lines with proper formatting
        assert!(lines.iter().any(|line| line.starts_with("set_min_delay")));
        assert!(lines.iter().any(|line| line.starts_with("set_max_delay")));
        assert!(lines.iter().any(|line| line.starts_with("\t-through")));
    }

    /// Test SDC generation for cyclic circuit constraints (based on cyclic.graph)
    #[test]
    fn test_sdc_cyclic_circuit_constraints() {
        let mut constraints = HashMap::new();
        
        // Port to DataReg constraint (input to feedback register)
        constraints.insert(
            (
                CircuitNode::Port("a".to_string()),
                CircuitNode::Register {
                    name: "b".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: Some(5.0),
                max: Some(20.0),
            },
        );

        // DataReg to DataReg constraint (feedback loop)
        constraints.insert(
            (
                CircuitNode::Register {
                    name: "b".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
                CircuitNode::Register {
                    name: "b".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: Some(3.0),
                max: Some(15.0),
            },
        );

        // DataReg to Port constraint (feedback register to output)
        constraints.insert(
            (
                CircuitNode::Register {
                    name: "b".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
                CircuitNode::Port("c".to_string()),
            ),
            DelayPair {
                min: Some(2.0),
                max: Some(10.0),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write cyclic SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should contain all constraint types for cyclic circuit
        assert!(sdc_content.contains("set_min_delay 5.000"));
        assert!(sdc_content.contains("set_max_delay 20.000"));
        assert!(sdc_content.contains("set_min_delay 3.000"));
        assert!(sdc_content.contains("set_max_delay 15.000"));
        assert!(sdc_content.contains("set_min_delay 2.000"));
        assert!(sdc_content.contains("set_max_delay 10.000"));

        // Should contain proper node references
        assert!(sdc_content.contains("a_*"));
        assert!(sdc_content.contains("b/*"));
        assert!(sdc_content.contains("c_*"));

        // Should contain sequential constraints for DataReg
        assert!(sdc_content.contains("is_sequential == true"));
    }

    /// Test SDC generation for complex cyclic circuit with multiple feedback loops
    #[test]
    fn test_sdc_complex_cyclic_constraints() {
        let mut constraints = HashMap::new();
        
        // Clock to registers
        constraints.insert(
            (
                CircuitNode::Port("clk".to_string()),
                CircuitNode::Register {
                    name: "reg1".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: None,
                max: Some(5.0),
            },
        );

        constraints.insert(
            (
                CircuitNode::Port("clk".to_string()),
                CircuitNode::Register {
                    name: "reg2".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: None,
                max: Some(5.0),
            },
        );

        // Feedback loops between registers
        constraints.insert(
            (
                CircuitNode::Register {
                    name: "reg1".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
                CircuitNode::Register {
                    name: "reg2".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: Some(10.0),
                max: Some(25.0),
            },
        );

        constraints.insert(
            (
                CircuitNode::Register {
                    name: "reg2".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
                CircuitNode::Register {
                    name: "reg1".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: Some(8.0),
                max: Some(20.0),
            },
        );

        // Register to output
        constraints.insert(
            (
                CircuitNode::Register {
                    name: "reg1".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
                CircuitNode::Port("output".to_string()),
            ),
            DelayPair {
                min: Some(5.0),
                max: Some(15.0),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write complex cyclic SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should contain all constraint types
        assert!(sdc_content.contains("set_max_delay 5.000"));
        assert!(sdc_content.contains("set_min_delay 10.000"));
        assert!(sdc_content.contains("set_max_delay 25.000"));
        assert!(sdc_content.contains("set_min_delay 8.000"));
        assert!(sdc_content.contains("set_max_delay 20.000"));
        assert!(sdc_content.contains("set_min_delay 5.000"));
        assert!(sdc_content.contains("set_max_delay 15.000"));

        // Should contain proper node references
        assert!(sdc_content.contains("clk_*"));
        assert!(sdc_content.contains("reg1/*"));
        assert!(sdc_content.contains("reg2/*"));
        assert!(sdc_content.contains("output_*"));

        // Should contain multiple sequential constraints
        let sequential_count = sdc_content.matches("is_sequential == true").count();
        assert!(sequential_count >= 4, "Should have multiple sequential constraints");
    }

    /// Test SDC generation for cyclic circuit with tight timing constraints
    #[test]
    fn test_sdc_cyclic_tight_timing() {
        let mut constraints = HashMap::new();
        
        // Tight timing constraints for cyclic circuit
        constraints.insert(
            (
                CircuitNode::Port("a".to_string()),
                CircuitNode::Register {
                    name: "b".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: Some(1.0),
                max: Some(3.0),
            },
        );

        constraints.insert(
            (
                CircuitNode::Register {
                    name: "b".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
                CircuitNode::Register {
                    name: "b".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: Some(0.5),
                max: Some(2.0),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write tight timing SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should contain tight timing constraints
        assert!(sdc_content.contains("set_min_delay 1.000"));
        assert!(sdc_content.contains("set_max_delay 3.000"));
        assert!(sdc_content.contains("set_min_delay 0.500"));
        assert!(sdc_content.contains("set_max_delay 2.000"));

        // Should have proper precision for tight timing
        assert!(sdc_content.contains("1.000"));
        assert!(sdc_content.contains("0.500"));
    }

    /// Test SDC generation for cyclic circuit with only max constraints
    #[test]
    fn test_sdc_cyclic_max_only_constraints() {
        let mut constraints = HashMap::new();
        
        // Cyclic circuit with only max delay constraints (pseudoclock style)
        constraints.insert(
            (
                CircuitNode::Port("input".to_string()),
                CircuitNode::Register {
                    name: "reg".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: None,
                max: Some(12.5),
            },
        );

        constraints.insert(
            (
                CircuitNode::Register {
                    name: "reg".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
                CircuitNode::Register {
                    name: "reg".to_string(),
                    data_path: "d".to_string(),
                    control_path: "c".to_string(),
                    output_path: "q".to_string(),
                },
            ),
            DelayPair {
                min: None,
                max: Some(8.75),
            },
        );

        let mut output = Cursor::new(Vec::new());
        write_path_constraints(&mut output, &constraints).expect("Should write max-only SDC");

        let sdc_content = String::from_utf8(output.into_inner()).expect("Should be valid UTF-8");

        // Should only contain max delay constraints
        assert!(sdc_content.contains("set_max_delay 12.500"));
        assert!(sdc_content.contains("set_max_delay 8.750"));
        assert!(!sdc_content.contains("set_min_delay"));

        // Should contain proper node references
        assert!(sdc_content.contains("input_*"));
        assert!(sdc_content.contains("reg/*"));
    }
}
