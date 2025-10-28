//! Benchmarks for HBCN constraint generation
//!
//! This benchmark suite tests constraint generation performance on various structural graphs
//! using both pseudoclock and proportional constraint algorithms.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use hbcn::constrain::hbcn::{constrain_cycle_time_proportional, constrain_cycle_time_pseudoclock};
use hbcn::{from_structural_graph, read_file};
use std::path::Path;

/// Graph files available for benchmarking
const GRAPH_FILES: &[(&str, &str)] = &[
    ("ARV", "examples/structural_graphs/ARV.graph"),
    ("cyclic", "examples/structural_graphs/cyclic.graph"),
    ("mac4", "examples/structural_graphs/mac4.graph"), // MAC with 4 stages in inner loop
    ("mac5", "examples/structural_graphs/mac5.graph"), // MAC with 5 stages in inner loop
    ("mac6", "examples/structural_graphs/mac6.graph"), // MAC with 6 stages in inner loop
    ("test", "examples/structural_graphs/test.graph"),
];

/// Standard benchmark parameters
#[derive(Debug, Clone, Copy)]
struct BenchmarkParams {
    cycle_time: f64,
    minimal_delay: f64,
    backward_margin: Option<f64>,
    forward_margin: Option<f64>,
}

impl Default for BenchmarkParams {
    fn default() -> Self {
        Self {
            cycle_time: 10.0,           // 10ns cycle time
            minimal_delay: 0.1,         // 100ps minimal delay
            backward_margin: Some(0.9), // 10% margin
            forward_margin: Some(0.9),  // 10% margin
        }
    }
}

/// Load and parse a structural graph file
fn load_graph(
    file_path: &str,
) -> Result<hbcn::structural_graph::StructuralGraph, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("Graph file not found: {}", file_path).into());
    }
    Ok(read_file(path)?)
}

/// Convert structural graph to HBCN
fn prepare_hbcn(
    graph: &hbcn::structural_graph::StructuralGraph,
    forward_completion: bool,
) -> Option<hbcn::hbcn::StructuralHBCN> {
    from_structural_graph(graph, forward_completion)
}

/// Benchmark pseudoclock constraint generation
fn bench_pseudoclock_constraints(c: &mut Criterion) {
    let mut group = c.benchmark_group("pseudoclock_constraints");

    for &(name, file_path) in GRAPH_FILES {
        // Try to load the graph file
        let graph = match load_graph(file_path) {
            Ok(g) => g,
            Err(e) => {
                eprintln!(
                    "Warning: Could not load {}: {}. Skipping benchmark.",
                    file_path, e
                );
                continue;
            }
        };

        let hbcn = match prepare_hbcn(&graph, true) {
            Some(h) => h,
            None => {
                eprintln!(
                    "Warning: Could not convert {} to HBCN. Skipping benchmark.",
                    name
                );
                continue;
            }
        };

        let params = BenchmarkParams::default();

        // Count nodes and edges for throughput measurement
        let node_count = hbcn.node_count();
        let edge_count = hbcn.edge_count();
        group.throughput(Throughput::Elements((node_count + edge_count) as u64));

        group.bench_with_input(
            BenchmarkId::new("pseudoclock", name),
            &(hbcn, params),
            |b, (hbcn, params)| {
                b.iter(|| {
                    black_box(constrain_cycle_time_pseudoclock(
                        black_box(hbcn),
                        black_box(params.cycle_time),
                        black_box(params.minimal_delay),
                    ))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark proportional constraint generation
fn bench_proportional_constraints(c: &mut Criterion) {
    let mut group = c.benchmark_group("proportional_constraints");

    for &(name, file_path) in GRAPH_FILES {
        // Try to load the graph file
        let graph = match load_graph(file_path) {
            Ok(g) => g,
            Err(e) => {
                eprintln!(
                    "Warning: Could not load {}: {}. Skipping benchmark.",
                    file_path, e
                );
                continue;
            }
        };

        let hbcn = match prepare_hbcn(&graph, true) {
            Some(h) => h,
            None => {
                eprintln!(
                    "Warning: Could not convert {} to HBCN. Skipping benchmark.",
                    name
                );
                continue;
            }
        };

        let params = BenchmarkParams::default();

        // Count nodes and edges for throughput measurement
        let node_count = hbcn.node_count();
        let edge_count = hbcn.edge_count();
        group.throughput(Throughput::Elements((node_count + edge_count) as u64));

        group.bench_with_input(
            BenchmarkId::new("proportional", name),
            &(hbcn, params),
            |b, (hbcn, params)| {
                b.iter(|| {
                    black_box(constrain_cycle_time_proportional(
                        black_box(hbcn),
                        black_box(params.cycle_time),
                        black_box(params.minimal_delay),
                        black_box(params.backward_margin),
                        black_box(params.forward_margin),
                    ))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark algorithm comparison (pseudoclock vs proportional) on selected graphs
fn bench_algorithm_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("algorithm_comparison");

    // Select a few representative graphs for comparison
    let comparison_graphs = &[
        ("mac4", "examples/structural_graphs/mac4.graph"),
        ("mac6", "examples/structural_graphs/mac6.graph"),
        ("ARV", "examples/structural_graphs/ARV.graph"),
    ];

    for &(name, file_path) in comparison_graphs {
        let graph = match load_graph(file_path) {
            Ok(g) => g,
            Err(e) => {
                eprintln!(
                    "Warning: Could not load {}: {}. Skipping comparison.",
                    file_path, e
                );
                continue;
            }
        };

        let hbcn = match prepare_hbcn(&graph, true) {
            Some(h) => h,
            None => {
                eprintln!(
                    "Warning: Could not convert {} to HBCN. Skipping comparison.",
                    name
                );
                continue;
            }
        };

        let params = BenchmarkParams::default();

        // Benchmark pseudoclock
        group.bench_with_input(
            BenchmarkId::new("pseudoclock", name),
            &(hbcn.clone(), params),
            |b, (hbcn, params)| {
                b.iter(|| {
                    black_box(constrain_cycle_time_pseudoclock(
                        black_box(hbcn),
                        black_box(params.cycle_time),
                        black_box(params.minimal_delay),
                    ))
                })
            },
        );

        // Benchmark proportional
        group.bench_with_input(
            BenchmarkId::new("proportional", name),
            &(hbcn, params),
            |b, (hbcn, params)| {
                b.iter(|| {
                    black_box(constrain_cycle_time_proportional(
                        black_box(hbcn),
                        black_box(params.cycle_time),
                        black_box(params.minimal_delay),
                        black_box(params.backward_margin),
                        black_box(params.forward_margin),
                    ))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark graph parsing and HBCN conversion
fn bench_graph_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_parsing");

    for &(name, file_path) in GRAPH_FILES {
        let path = Path::new(file_path);
        if !path.exists() {
            eprintln!(
                "Warning: Graph file not found: {}. Skipping parsing benchmark.",
                file_path
            );
            continue;
        }

        group.bench_with_input(
            BenchmarkId::new("parse", name),
            &file_path,
            |b, file_path| b.iter(|| black_box(read_file(black_box(Path::new(file_path))))),
        );

        // Also benchmark HBCN conversion
        if let Ok(graph) = read_file(path) {
            group.bench_with_input(
                BenchmarkId::new("convert_to_hbcn", name),
                &graph,
                |b, graph| b.iter(|| black_box(prepare_hbcn(black_box(graph), black_box(true)))),
            );
        }
    }

    group.finish();
}

/// Benchmark parameter sensitivity (different cycle times and margins)
fn bench_parameter_sensitivity(c: &mut Criterion) {
    let mut group = c.benchmark_group("parameter_sensitivity");

    // Use a representative graph for parameter sensitivity analysis
    let test_graph_path = "examples/structural_graphs/mac6.graph";

    let graph = match load_graph(test_graph_path) {
        Ok(g) => g,
        Err(e) => {
            eprintln!(
                "Warning: Could not load {} for parameter sensitivity: {}. Skipping.",
                test_graph_path, e
            );
            return;
        }
    };

    let hbcn = match prepare_hbcn(&graph, true) {
        Some(h) => h,
        None => {
            eprintln!(
                "Warning: Could not convert graph to HBCN for parameter sensitivity. Skipping."
            );
            return;
        }
    };

    // Test different cycle times
    let cycle_times = [5.0, 10.0, 15.0, 20.0];
    for &cycle_time in &cycle_times {
        let params = BenchmarkParams {
            cycle_time,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::new("cycle_time", format!("{:.1}ns", cycle_time)),
            &(hbcn.clone(), params),
            |b, (hbcn, params)| {
                b.iter(|| {
                    black_box(constrain_cycle_time_proportional(
                        black_box(hbcn),
                        black_box(params.cycle_time),
                        black_box(params.minimal_delay),
                        black_box(params.backward_margin),
                        black_box(params.forward_margin),
                    ))
                })
            },
        );
    }

    // Test different margin values
    let margins = [0.95, 0.9, 0.85, 0.8]; // 5%, 10%, 15%, 20% margins
    for &margin in &margins {
        let params = BenchmarkParams {
            backward_margin: Some(margin),
            forward_margin: Some(margin),
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::new("margin", format!("{:.0}%", (1.0 - margin) * 100.0)),
            &(hbcn.clone(), params),
            |b, (hbcn, params)| {
                b.iter(|| {
                    black_box(constrain_cycle_time_proportional(
                        black_box(hbcn),
                        black_box(params.cycle_time),
                        black_box(params.minimal_delay),
                        black_box(params.backward_margin),
                        black_box(params.forward_margin),
                    ))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_parsing,
    bench_pseudoclock_constraints,
    bench_proportional_constraints,
    bench_algorithm_comparison,
    bench_parameter_sensitivity
);

criterion_main!(benches);
