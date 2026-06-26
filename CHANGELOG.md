# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Negative delays in the HBCN format**: a `.hbcn` place delay may now be negative
  (a physically real effect, e.g. slew/recovery). `analyse` honours it as a real
  delay, so a negative place delay lowers the computed cycle time instead of being
  floored at zero. The structural `.graph` format is unchanged (still non-negative),
  and `constrain` continues to read the value as a logical-depth weight (a small or
  negative weight makes the path non-critical, assigned the smallest legal constraint).
- **Input-format reference** (`docs/INPUT_FORMATS.md`): a dedicated specification of the two input formats — the structural graph (`.graph`), covering all five component types (`Port`, `NullReg`, `ControlReg`, `DataReg`, `UnsafeReg`) and the name/adjacency/virtual-delay rules, and the HBCN (`.hbcn`) place/transition/token/delay grammar — with verifiable examples drawn from `examples/`.
- **Constraint-generation reference** (`docs/CONSTRAINING.md`): documents every `constrain` flag, the proportional and pseudoclock algorithms with their LP formulations, the margin (`-f`/`-b`) and forward-completion options, and the SDC/CSV/report outputs.
- **Disentangled per-place timing constraints**: the four places of an HBCN channel — data
  propagation (`+→+`), spacer propagation (`−→−`), and the two acknowledges (`+→−`, `−→+`) — now
  carry independent delays through the `constrain` LP and are emitted as separate SDC statements.
  Each `set_max_delay`/`set_min_delay` `-through` clause is qualified by its endpoint's transition
  direction — a `Data` (`+`) transition is a rise at its register/port, a `Spacer` (`−`) a fall — so
  the positive-unate propagation paths are rise→rise / fall→fall and the negative-unate acknowledges
  rise→fall / fall→rise. Previously the two same-node-pair places shared one delay variable and
  collapsed into a single un-annotated `-through` constraint (which also forced the proportional
  solver's `factor` to zero on distinct per-place weights). The structural `.graph` expansion is
  unchanged (one virtual delay per channel, duplicated across the propagation places); distinct
  per-place delays come from a characterised `.hbcn` (see [`examples/hbcn/distinct.hbcn`](examples/hbcn/distinct.hbcn)).
  The `constrain` CSV gains `src_dir`/`dst_dir` columns.
- **Default token for unmarked HBCN channels**: a `.hbcn` channel that marks no place is now
  accepted — the parser inserts a default token at its spacer-acknowledge place
  (`Spacer(b) → Data(a)`, the canonical reset position the structural expansion already uses for
  external channels) before validation. A channel that marks more than one place is still rejected,
  and the structural `.graph` path is unaffected (it always marks every channel). See
  [`examples/hbcn/unmarked.hbcn`](examples/hbcn/unmarked.hbcn).

### Changed
- **LP solver abstraction extracted to a crate**: the in-repo `lp_solver` module was replaced by a dependency on the published [`lp_solver`](https://github.com/marlls1989/lp_solver) crate. The `coin_cbc`/`gurobi` features now forward to it.
- **Solver-selection environment variable renamed** (breaking): `HBCN_LP_SOLVER` → `LP_SOLVER`.

### Removed
- **LP solver output suppression**: the `gag`-based redirection of CBC/Gurobi stdout (and the `output_suppression` module, now `verbose`) was removed along with the in-repo solver. Solver banners print to stdout; the generated artifacts (SDC, reports, VCD, DOT, CSV) are written to files and are unaffected. `--verbose` now only toggles hbcn's own progress messages.
- Dropped the `gag` dependency and the LP-solver demo examples.

### Fixed
- **LP solver precision adjustment restored**: the in-repo solver abstraction used to round every
  returned value to 8 significant digits ("a workaround to mask floating point errors in CBC"); that
  adjustment was lost when the solver moved to the external `lp_solver` crate. `analyse`/`constrain`
  now apply it to the arrival times, delays, slacks, and objective they read from a solution, so raw
  solver noise (e.g. a delay of `2.9999999998` slipping below its `3.0` bound, or a cycle time read
  as `150.00000000000003`) no longer surfaces in results.
- **`constrain` pseudoclock flag spelling** (breaking): the misspelt `--no-proportinal` flag is now
  `--no-proportional`; the old spelling no longer works.

## [0.6.0] - 2025-10-28

### Added
- **Automatic fallback mechanism**: LP solver now automatically falls back from Gurobi to Coin CBC when Gurobi fails (e.g., license issues)
- **Enhanced macro system**: 
  - Implemented `lp_model_builder!()` macro for guaranteed unique branding
  - Added optional brand name parameter to `lp_model_builder!` macro for better type system clarity
  - Moved macros to dedicated `macros` submodule for better organisation
- **Report file support**: Added `--report/-r` option to `analyse` command for redirecting output to files (renamed from log to avoid confusion with hbcn.log)
- **New `expand` command**: Added `expand` command to convert structural graphs to HBCN representation
- **Enhanced `analyse` command**: Added `--structural` flag to read structural graphs directly and `--depth` flag to perform unweighted depth analysis
- **Comprehensive benchmarking**: Added Criterion benchmarking framework with performance tests for:
  - Graph parsing performance
  - Pseudoclock constraint generation
  - Proportional constraint generation
  - Algorithm comparison (pseudoclock vs proportional)
  - Parameter sensitivity analysis
- **Thread-safe output management**: Added thread-safe Gag singleton for output suppression
- **CI/CD pipeline**: Complete GitHub Actions workflow with automated testing, formatting, and clippy checks
- **Comprehensive operator overloading**: Added support for `VariableId + VariableId` and other combinations

### Changed
- **API Simplification** ⚠️ **BREAKING CHANGES**:
  - Removed `name` fields from `Constraint<Brand>` and `VariableInfo` structs
  - Simplified `add_variable()` method (now takes 3 parameters instead of 4)
  - Removed all `*_named` constructor functions (`eq_named`, `le_named`, etc.)
  - Simplified `constraint!` macro to only support unnamed constraints
- **Output behaviour**: LP solver output is now redirected to `hbcn.log`
- **Solver API**: Changed solver functions to take references instead of ownership for better efficiency
- **Code organisation**: Improved code formatting, organisation, and test coverage throughout the project
- **Minimum Rust version**: Updated MSRV to 1.86.0 (required for Rust 2024 edition)

### Fixed
- **Documentation**: Fixed doctests and removed ignore markings - all 13 doctests now pass
- **Code quality**: 
  - Fixed all clippy warnings across the entire codebase
  - Applied consistent formatting with `cargo fmt`
  - Resolved lalrpop-generated parser clippy warnings
- **CI/CD**: 
  - Fixed GitHub Actions workflow syntax errors
  - Resolved CI linker errors by adding required system dependencies
  - Fixed cargo build cache configuration
- **Import paths**: Fixed import paths in doctests to use correct module paths
- **Type inference**: Resolved type inference issues by using the new macro-based approach

### Improved
- **Error handling**: Better error messages showing actual brand names vs anonymous types
- **Testing**: Enhanced test suite coverage and organisation
- **Development experience**: 
  - Easier debugging and code documentation with named brands
  - Zero runtime overhead - brands remain phantom types
  - Cleaner API with same functionality
- **Build system**: Containerized CI environment for faster builds and consistent dependencies

### Infrastructure
- Added Dockerfile.ci with pre-installed CBC solver libraries
- GitHub Container Registry integration for image caching
- Comprehensive benchmark suite with convenience scripts (`run_benchmarks.sh`, `demo_benchmarks.sh`)
- Updated project structure with better organisation of examples and benchmarks

## [0.5.0] - Previous Release
*(Baseline for this changelog)*
