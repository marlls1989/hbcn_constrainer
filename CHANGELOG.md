# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
