# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`hbcn` is a Rust CLI + library that generates timing constraints for **asynchronous circuit synthesis** (targeting Cadence Genus) from **Half-Buffer Channel Network (HBCN)** models. It is part of the **Pulsar** framework. The tool reads a circuit description, models it as an HBCN marked graph, and uses linear programming to compute timing constraints and analyse cycle times.

## Build & test

Requires Rust 1.86+ (2024 edition) and an LP solver backend.

```bash
cargo build --release                              # default: coin_cbc backend
cargo build --release --features gurobi            # Gurobi backend (requires Gurobi + license)
cargo build --release --features "gurobi coin_cbc" # both, with runtime selection

cargo test                          # all unit + integration + doctests
cargo test --test integration_tests # integration suite only
cargo test test_simple_two_node     # single test by name substring
cargo test --doc                    # doctests only

cargo fmt --all -- --check          # CI gates on this
cargo clippy --all-targets --all-features -- -D warnings  # CI gates on this
cargo bench                         # Criterion benchmarks (see run_benchmarks.sh)
```

**LP solver requirement:** the LP backends live in the external [`lp_solver`](https://github.com/marlls1989/lp_solver) crate; `hbcn`'s `coin_cbc`/`gurobi` features simply forward to `lp_solver/coin_cbc` and `lp_solver/gurobi`. The default `coin_cbc` backend needs the CBC system library (`brew install coin-or-tools/coinor/cbc` on macOS; `apt install coinor-cbc` on Debian). At least one solver feature must be enabled or the build has no backend. CI builds inside `Dockerfile.ci` which pre-installs CBC.

**Solver selection at runtime:** set `LP_SOLVER=gurobi` or `LP_SOLVER=coin_cbc`. If unset and both are compiled in, the system tries Gurobi first and automatically falls back to Coin CBC on failure (e.g. license issues).

## CLI

Single binary `hbcn` with three subcommands (see `src/lib.rs` for arg definitions). The top-level `--verbose/-v` flag makes `analyse`/`constrain` print extra progress messages to stderr.

- `hbcn expand <graph> -o <out.hbcn>` — convert a structural graph to HBCN representation.
- `hbcn analyse <input> [--structural] [--depth] [-r rpt] [--vcd f] [--dot f]` — estimate virtual-delay cycle time (or unweighted cycle depth with `--depth`) and find critical cycles.
- `hbcn constrain <input> --sdc <out.sdc> -t <cycle_time> -m <min_delay> [--csv] [--rpt] [--vcd] ...` — generate SDC timing constraints.

By default `analyse`/`constrain` read the **HBCN** format; pass `--structural` to read a `.graph` structural graph instead.

## Input formats

**Structural graph** (`.graph`) — a node-per-line adjacency list. Each line is a component type, a quoted name, and a bracketed list of `(target, delay)` edges:
```
Port "input" [("output", 50)]
Port "output" []
```
Component types (`src/structural_graph/parser.lalrpop`): `Port`, `DataReg`, `NullReg`, `ControlReg`, `UnsafeReg`.

**HBCN** (`.hbcn`) — explicit transition list produced by `expand`. Lines describe places between signed transitions (`+`/`-`) with a weight; a leading `*` marks an initially-tokened place:
```
  +{a} => +{a/s0} : 10
* -{a} => -{a/s0} : 10
```

Both grammars are LALRPOP (`*.lalrpop`), compiled by `build.rs` (`lalrpop::process_root()`) at build time — there is no checked-in generated parser.

## Architecture

The pipeline is **structural graph → StructuralHBCN → LP model → SolvedHBCN → output**. Library entry points are re-exported from `src/lib.rs`; `src/main.rs` is a thin clap dispatcher.

- **`structural_graph/`** — parses `.graph` files into a `StructuralGraph`. `Symbol` (interned string via `string_cache`) is the node-name type used throughout.
- **`hbcn/`** — the core model. Defines `Transition` (Data/Spacer events at a `CircuitNode`) and `Place` edges (forward = same-type transitions, backward = different-type = handshake/ack paths; `token` = initial marking). Two graph types:
  - `StructuralHBCN` — structure before timing (`Transition` nodes, `Place` edges).
  - `SolvedHBCN` — after LP solve (`TransitionEvent` nodes with times, `DelayedPlace` edges with delays + slack).
  - `from_structural_graph(&graph, forward_completion)` builds the former from a parsed graph. `hbcn/parser/` parses the `.hbcn` text format; `hbcn/serialisation.rs` writes it.
- **`lp_solver`** (external crate, [github.com/marlls1989/lp_solver](https://github.com/marlls1989/lp_solver)) — solver-agnostic LP abstraction, formerly an in-repo module. `LPModelBuilder<Brand>`, `VariableId<Brand>`, `Constraint<Brand>`, `LinearExpression<Brand>` are generic over a **phantom `Brand` type** that makes mixing variables from different models a compile error (zero runtime cost). Create branded builders with the `lp_model_builder!()` macro (each call mints a unique brand); build constraints with the `constraint!` macro (only `==`/`<=`/`>=` — no strict inequalities). `solve()` returns `Result<LPSolution, SolveError>`: an `Ok` always carries a usable solution (`SolutionStatus::Optimal`/`Feasible`), while infeasible/unbounded/stopped come back as `Err(SolveError::NoSolution(_))`. Backends (CBC/Gurobi) are dispatched via the `LP_SOLVER` env var.
- **`analyse/`** — `analyse_main`: builds the LP, solves for cycle time (or depth), identifies minimal-slack critical cycles, emits reports / VCD / DOT.
- **`constrain/`** — `constrain_main`: generates timing constraints. Two algorithms — **proportional** (default, distributes cycle time across paths by virtual delay) and **pseudoclock** (`--no-proportional`). `constrain/sdc.rs` writes Genus-compatible SDC (`set_min_delay`/`set_max_delay -through`, `create_clock`). Also emits CSV/VCD/report.
- **`verbose.rs`** — a global `--verbose` flag (`is_verbose`/`set_verbose`) gating extra progress messages. Solver stdout (CBC/Gurobi banners) is no longer suppressed: the suppression used to live inside the in-repo solver `solve()`, which moved to the external `lp_solver` crate. The crate writes solver output to stdout; the deliverable artifacts (SDC, reports, VCD, DOT, CSV) are written to files and are unaffected.

Parallelism uses `rayon`; the graph backbone is `petgraph`.

## Conventions

- **British spelling** in identifiers, docs, and output (`analyse`, `serialise`, `optimisation`, `behaviour`). A recent commit deliberately reverted American spellings — match the surrounding code.
- When adding constraints to an LP, the **branded type system** will reject variables from the wrong builder at compile time; use the `lp_model_builder!()` / `constraint!` macros rather than constructing builders manually.
- Tests: unit tests live in-module (`mod tests`) or in sibling files (`constrain/tests.rs`, `constrain/sdc_tests.rs`); end-to-end tests in `tests/integration_tests.rs` call the **library API directly** (not the binary) for speed, and use `--structural` graph inputs via `tempfile`. See `TESTING.md` for the full catalogue.
