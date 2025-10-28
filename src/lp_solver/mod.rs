//! Linear Programming (LP) solver abstraction layer
//!
//! This module provides a trait-based abstraction for LP solvers, allowing the codebase
//! to be independent of specific solver implementations like Gurobi and coin_cbc.
//!
//! # Type Safety with Branded Types
//!
//! All core types (`VariableId`, `LinearExpression`, `Constraint`, `LPModelBuilder`)
//! use a generic `Brand` type parameter that provides compile-time guarantees:
//!
//! - Variables from one builder cannot be accidentally used with another builder
//! - Constraints are type-checked to ensure they only use variables from their builder
//! - No runtime overhead - the brand is a zero-sized phantom type
//!
//! ## Using the Default Brand
//!
//! For simple cases where you only have one model, use the default brand `()`:
//!
//! ```rust
//! use hbcn::lp_solver::{LPModelBuilder, VariableType};
//! use hbcn::constraint;
//!
//! let mut builder: LPModelBuilder<()> = LPModelBuilder::new();
//! let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//! let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//!
//! // All operations work as expected
//! builder.add_constraint(constraint!((x + y) <= 10.0));
//! ```
//!
//! **Note:** Type inference may require explicit annotation in some cases.
//!
//! ## Branded Types for Type Safety
//!
//! All core types are generic over a `Brand` type parameter to prevent mixing
//! variables from different LP models at compile time. Use the `lp_model_builder!()`
//! macro to create builders with guaranteed unique brands:
//!
//! ```rust
//! use hbcn::constraint;
//! use hbcn::lp_model_builder;
//! use hbcn::lp_solver::VariableType;
//!
//! // Each macro call creates a unique brand automatically
//! let mut builder1 = lp_model_builder!();
//! let mut builder2 = lp_model_builder!();
//!
//! let x = builder1.add_variable(VariableType::Continuous, 0.0, 10.0);
//! let y = builder2.add_variable(VariableType::Continuous, 0.0, 10.0);
//!
//! // This compiles:
//! builder1.add_constraint(constraint!((x) <= 5.0));
//!
//! // This would NOT compile (type error):
//! // builder1.add_constraint(constraint!((y) <= 5.0));
//! // ERROR: y has different brand than builder1 expects
//! ```
//!
//! For custom brands, you can still use the explicit generic syntax:
//! ```rust
//! use hbcn::lp_solver::{LPModelBuilder, VariableType};
//!
//! struct MyModel;
//! let mut builder = LPModelBuilder::<MyModel>::new();
//! ```
//!
//! ## Implementation Details
//!
//! ### Internal Data Structures
//!
//! The `LPModelBuilder` uses clean, strongly-typed data structures internally:
//!
//! - `VariableInfo`: Stores variable metadata (name, type, bounds)
//! - `ConstraintInfo<Brand>`: Stores constraint details (name, expression, sense, RHS)
//! - `ObjectiveInfo<Brand>`: Stores objective function information
//!
//! Variables and solutions use `Vec` storage rather than `HashMap`:
//! - `LPModelBuilder.variables`: Vec of `VariableInfo`
//! - `LPSolution.variable_values`: Vec of `f64`
//!
//! The `VariableId` serves as an index into these vectors, providing O(1) lookups
//! without hashing overhead. Use `solution.get_value(var_id)` to safely access values.
//!
//! ### Type System Guarantees
//!
//! The type system enforces these invariants:
//!
//! 1. **Variable Binding**: `VariableId<Brand>` can only be used with `LPModelBuilder<Brand>`
//! 2. **Expression Consistency**: All variables in a `LinearExpression<Brand>` have the same brand
//! 3. **Constraint Matching**: `Constraint<Brand>` can only be added to matching `LPModelBuilder<Brand>`
//! 4. **Operation Preservation**: All arithmetic operations preserve the brand type
//!
//! ### Manual Trait Implementations
//!
//! To avoid requiring `Brand` to implement any traits, `VariableId<Brand>` manually implements
//! `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, and `Hash`. These implementations only use the
//! `id` field, ignoring the phantom `_brand` field.
//!
//! ### Zero Runtime Cost
//!
//! The `PhantomData<fn() -> Brand>` is a zero-sized type that exists only at compile time.
//! Both `VariableId<()>` and `VariableId<CustomBrand>` have the same size: just `size_of::<usize>()`.
//!
//! # Building LP Models
//!
//! The module provides three main ways to build constraints:
//!
//! ## 1. Using the `constraint!` Macro (Recommended)
//!
//! The most concise way to create constraints using natural comparison syntax:
//!
//! ```rust,no_run
//! use hbcn::constraint;
//! use hbcn::lp_model_builder;
//! use hbcn::lp_solver::{VariableType, OptimizationSense};
//!
//! let mut builder = lp_model_builder!();
//! let x = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);
//! let y = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);
//!
//! // Unnamed constraints (most common)
//! builder.add_constraint(constraint!((x + y) == 10.0));
//! builder.add_constraint(constraint!((2.0 * x - y) <= 5.0));
//! builder.add_constraint(constraint!((x) >= 0.0));
//! builder.add_constraint(constraint!((y) > 1.0));
//!
//!
//! // Set objective and solve
//! builder.set_objective(x + 2.0 * y, OptimizationSense::Maximize);
//! let _solution = builder.solve();
//! ```
//!
//! ## 2. Using `Constraint` Builder Methods
//!
//! For explicit constraint construction:
//!
//! ```rust,no_run
//! use hbcn::lp_model_builder;
//! use hbcn::lp_solver::{Constraint, VariableType};
//!
//! let mut builder = lp_model_builder!();
//! let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//!
//! // Unnamed (cleaner)
//! builder.add_constraint(Constraint::eq(x + 5.0, 10.0));
//! builder.add_constraint(Constraint::le(2.0 * x, 20.0));
//! builder.add_constraint(Constraint::ge(x, 0.0));
//! builder.add_constraint(Constraint::gt(x, 1.0));
//!
//! ```
//!
//! ## 3. Using `Constraint::new` Directly
//!
//! For maximum control:
//!
//! ```rust,no_run
//! use hbcn::lp_model_builder;
//! use hbcn::lp_solver::{Constraint, ConstraintSense, VariableType};
//!
//! let mut builder = lp_model_builder!();
//! let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//! let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//!
//! let c = Constraint::new(x + y, ConstraintSense::Equal, 10.0);
//! builder.add_constraint(c);
//! ```
//!
//! # Expression Building
//!
//! Linear expressions support natural operator overloading:
//!
//! ```rust,no_run
//! use hbcn::lp_model_builder;
//! use hbcn::lp_solver::VariableType;
//!
//! let mut builder = lp_model_builder!();
//! let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//! let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//!
//! // Combine variables and constants
//! let _expr = 2.0 * x + 3.0 * y - 5.0;
//! let _expr2 = x + y;
//! let _expr3 = x - y + 10.0;
//! ```
//!
//! # Solver Selection
//!
//! The solver backend can be selected via the `HBCN_LP_SOLVER` environment variable:
//! - `"gurobi"` - Use Gurobi (requires `gurobi` feature)
//! - `"coin_cbc"` or `"cbc"` - Use COIN-OR CBC (requires `coin_cbc` feature)
//!
//! If not set, the solver defaults to Gurobi if available, otherwise CBC.

use anyhow::Result;
use std::env;
use std::marker::PhantomData;

/// Variable types supported by LP solvers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum VariableType {
    /// Continuous variable (can take any real value)
    Continuous,
    /// Integer variable (can only take integer values)
    Integer,
    /// Binary variable (can only take values 0 or 1)
    Binary,
}

/// Constraint sense for linear constraints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ConstraintSense {
    /// Less than or equal to (≤)
    LessEqual,
    /// Equal to (=)
    Equal,
    /// Greater than or equal to (≥)
    GreaterEqual,
    /// Strictly greater than (>)
    Greater,
}

/// Optimization direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationSense {
    /// Minimize the objective function
    Minimize,
    /// Maximize the objective function
    Maximize,
}

/// Status of the optimization process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum OptimizationStatus {
    /// Optimal solution found
    Optimal,
    /// Feasible solution found, but not necessarily optimal
    Feasible,
    /// Problem is infeasible (no solution exists)
    Infeasible,
    /// Problem is unbounded
    Unbounded,
    /// Problem is infeasible or unbounded
    InfeasibleOrUnbounded,
    /// Other status (solver-specific)
    Other(&'static str),
}

/// Available LP solver backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum SolverBackend {
    #[cfg(feature = "gurobi")]
    /// Gurobi commercial solver
    Gurobi,
    #[cfg(feature = "coin_cbc")]
    /// Coin CBC open-source solver
    CoinCbc,
}

impl SolverBackend {
    /// Get the solver backend from environment variable or use fallback logic
    fn from_env_or_default() -> Result<Self> {
        // Check if HBCN_LP_SOLVER environment variable is set
        if let Ok(solver_name) = env::var("HBCN_LP_SOLVER") {
            match solver_name.to_lowercase().as_str() {
                "gurobi" => {
                    #[cfg(feature = "gurobi")]
                    return Ok(SolverBackend::Gurobi);
                    #[cfg(not(feature = "gurobi"))]
                    return Err(anyhow::anyhow!(
                        "Gurobi solver requested via HBCN_LP_SOLVER but gurobi feature not enabled"
                    ));
                }
                "coin_cbc" | "coin-cbc" | "cbc" => {
                    #[cfg(feature = "coin_cbc")]
                    return Ok(SolverBackend::CoinCbc);
                    #[cfg(not(feature = "coin_cbc"))]
                    return Err(anyhow::anyhow!(
                        "Coin CBC solver requested via HBCN_LP_SOLVER but coin_cbc feature not enabled"
                    ));
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid solver '{}' in HBCN_LP_SOLVER. Valid options: gurobi, coin_cbc",
                        solver_name
                    ));
                }
            }
        }

        // Fallback logic: prefer gurobi if available, then coin_cbc
        #[cfg(feature = "gurobi")]
        return Ok(SolverBackend::Gurobi);

        #[allow(unreachable_code)]
        #[cfg(feature = "coin_cbc")]
        return Ok(SolverBackend::CoinCbc);

        #[cfg(not(any(feature = "gurobi", feature = "coin_cbc")))]
        Err(anyhow::anyhow!(
            "No LP solver backend available. Please enable a solver feature (e.g., 'gurobi' or 'coin_cbc')"
        ))
    }
}

/// A linear expression term: coefficient * variable
#[derive(Debug, Clone)]
pub struct LinearTerm<Brand> {
    pub coefficient: f64,
    pub variable: VariableId<Brand>,
}

/// A linear expression: sum of terms plus constant
#[derive(Debug, Clone)]
pub struct LinearExpression<Brand> {
    pub terms: Vec<LinearTerm<Brand>>,
    pub constant: f64,
}

impl<Brand> LinearExpression<Brand> {
    /// Create a new linear expression with a constant term
    pub fn new(constant: f64) -> Self {
        Self {
            terms: Vec::new(),
            constant,
        }
    }

    /// Add a term to the expression
    pub fn add_term(&mut self, coefficient: f64, variable: VariableId<Brand>) {
        self.terms.push(LinearTerm {
            coefficient,
            variable,
        });
    }

    /// Create a linear expression from a single variable
    pub fn from_variable(variable: VariableId<Brand>) -> Self {
        Self {
            terms: vec![LinearTerm {
                coefficient: 1.0,
                variable,
            }],
            constant: 0.0,
        }
    }
}

impl<Brand> From<VariableId<Brand>> for LinearExpression<Brand> {
    fn from(variable: VariableId<Brand>) -> Self {
        Self::from_variable(variable)
    }
}

/// Unique identifier for a variable in the LP model
///
/// The `Brand` type parameter ensures that variables can only be used with the
/// builder that created them. This is enforced at compile time.
pub struct VariableId<Brand> {
    id: usize,
    _brand: PhantomData<fn() -> Brand>,
}

// Manual trait implementations that don't require Brand to implement anything
impl<Brand> std::fmt::Debug for VariableId<Brand> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VariableId").field("id", &self.id).finish()
    }
}

impl<Brand> Clone for VariableId<Brand> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Brand> Copy for VariableId<Brand> {}

impl<Brand> PartialEq for VariableId<Brand> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<Brand> Eq for VariableId<Brand> {}

impl<Brand> std::hash::Hash for VariableId<Brand> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// Unique identifier for a constraint in the LP model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstraintId(usize);

/// A linear constraint representation
///
/// Constraints define relationships between linear expressions and constants.
/// The `Brand` type parameter ensures type safety - constraints can only use
/// variables from the builder that will consume them.
///
/// # Examples
///
/// ```rust,no_run
/// use hbcn::constraint;
/// use hbcn::lp_model_builder;
/// use hbcn::lp_solver::{Constraint, ConstraintSense, VariableType};
///
/// let mut builder = lp_model_builder!();
/// let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
/// let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
///
/// // Using the constraint! macro (recommended)
/// let c = constraint!((x + y) == 10.0);
///
/// // Using builder methods
/// let c = Constraint::eq(x + y, 10.0);
///
/// // Using the constructor directly
/// let c = Constraint::new(x + y, ConstraintSense::Equal, 10.0);
/// ```
#[derive(Debug, Clone)]
pub struct Constraint<Brand> {
    expression: LinearExpression<Brand>,
    sense: ConstraintSense,
    rhs: f64,
}

impl<Brand> Constraint<Brand> {
    /// Create a new constraint
    pub fn new(
        expression: impl Into<LinearExpression<Brand>>,
        sense: ConstraintSense,
        rhs: f64,
    ) -> Self {
        Self {
            expression: expression.into(),
            sense,
            rhs,
        }
    }

    /// Create an equality constraint: expression == rhs
    pub fn eq(expression: impl Into<LinearExpression<Brand>>, rhs: f64) -> Self {
        Self::new(expression, ConstraintSense::Equal, rhs)
    }

    /// Create a less-than-or-equal constraint: expression <= rhs
    pub fn le(expression: impl Into<LinearExpression<Brand>>, rhs: f64) -> Self {
        Self::new(expression, ConstraintSense::LessEqual, rhs)
    }

    /// Create a greater-than-or-equal constraint: expression >= rhs
    pub fn ge(expression: impl Into<LinearExpression<Brand>>, rhs: f64) -> Self {
        Self::new(expression, ConstraintSense::GreaterEqual, rhs)
    }

    /// Create a strictly-greater-than constraint: expression > rhs
    pub fn gt(expression: impl Into<LinearExpression<Brand>>, rhs: f64) -> Self {
        Self::new(expression, ConstraintSense::Greater, rhs)
    }
}

/// Variable information stored in the model
#[derive(Debug, Clone)]
struct VariableInfo {
    var_type: VariableType,
    lower_bound: f64,
    upper_bound: f64,
}

/// Objective function information
#[derive(Debug, Clone)]
struct ObjectiveInfo<Brand> {
    expression: LinearExpression<Brand>,
    sense: OptimizationSense,
}

/// Result of solving an LP model
#[derive(Debug, Clone)]
pub struct LPSolution<Brand> {
    pub status: OptimizationStatus,
    pub objective_value: f64,
    variable_values: Vec<f64>,
    _brand: PhantomData<fn() -> Brand>,
}

impl<Brand> LPSolution<Brand> {
    /// Get the value of a variable from the solution
    pub fn get_value(&self, var_id: VariableId<Brand>) -> Option<f64> {
        self.variable_values.get(var_id.id).copied()
    }
}

/// Builder for LP models that can work with different backends
///
/// The `Brand` type parameter ensures type safety - variables from one builder
/// cannot be accidentally used with another builder. This is enforced at compile time.
///
/// # Examples
///
/// ```rust,no_run
/// use hbcn::lp_model_builder;
/// use hbcn::lp_solver::{LPModelBuilder, VariableType};
///
/// // Each builder has its own brand
/// struct MyModel;
/// let mut builder1 = LPModelBuilder::<MyModel>::new();
///
/// // Variables are branded with the builder type
/// let x = builder1.add_variable(VariableType::Continuous, 0.0, 10.0);
///
/// // For simple cases, use the macro to create a unique brand
/// let mut builder2 = lp_model_builder!();  // Creates unique brand automatically
/// ```
pub struct LPModelBuilder<Brand> {
    variables: Vec<VariableInfo>,
    constraints: Vec<Constraint<Brand>>,
    objective: Option<ObjectiveInfo<Brand>>,
    _brand: PhantomData<fn() -> Brand>,
}

impl<Brand> LPModelBuilder<Brand> {
    /// Create a new LP model builder
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
            constraints: Vec::new(),
            objective: None,
            _brand: PhantomData,
        }
    }

    /// Add a variable to the model
    pub fn add_variable(
        &mut self,
        var_type: VariableType,
        lower_bound: f64,
        upper_bound: f64,
    ) -> VariableId<Brand> {
        let var_id = VariableId {
            id: self.variables.len(),
            _brand: PhantomData,
        };
        self.variables.push(VariableInfo {
            var_type,
            lower_bound,
            upper_bound,
        });
        var_id
    }

    /// Add a constraint to the model
    pub fn add_constraint(&mut self, constraint: Constraint<Brand>) -> ConstraintId {
        let constr_id = ConstraintId(self.constraints.len());
        self.constraints.push(constraint);
        constr_id
    }

    /// Set the objective function
    pub fn set_objective(&mut self, expression: LinearExpression<Brand>, sense: OptimizationSense) {
        self.objective = Some(ObjectiveInfo { expression, sense });
    }

    /// Solve the model using the specified solver
    pub fn solve(self) -> Result<LPSolution<Brand>> {
        let solver = SolverBackend::from_env_or_default()?;

        match solver {
            #[cfg(feature = "gurobi")]
            SolverBackend::Gurobi => crate::lp_solver::gurobi::solve_gurobi(self),

            #[cfg(feature = "coin_cbc")]
            SolverBackend::CoinCbc => crate::lp_solver::coin_cbc::solve_coin_cbc(self),
        }
    }
}

impl<Brand> Default for LPModelBuilder<Brand> {
    fn default() -> Self {
        Self::new()
    }
}

// Macros for convenient syntax
pub mod macros;

// Operator overloading for linear expressions
pub mod ops;

#[cfg(feature = "gurobi")]
pub mod gurobi;

#[cfg(feature = "coin_cbc")]
pub mod coin_cbc;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constraint, lp_model_builder};

    #[test]
    fn test_constraint_macro() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Test constraint creation
        let c = constraint!((x + y) == 10.0);
        assert_eq!(c.sense, ConstraintSense::Equal);
        assert_eq!(c.rhs, 10.0);

        let c = constraint!((2.0 * x) <= 5.0);
        assert_eq!(c.sense, ConstraintSense::LessEqual);
        assert_eq!(c.rhs, 5.0);

        let c = constraint!((x - y) >= 0.0);
        assert_eq!(c.sense, ConstraintSense::GreaterEqual);
        assert_eq!(c.rhs, 0.0);

        let c = constraint!((x) > 1.0);
        assert_eq!(c.sense, ConstraintSense::Greater);
        assert_eq!(c.rhs, 1.0);
    }

    #[test]
    fn test_constraint_macro_with_builder() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Test that constraints can be added to builder
        builder.add_constraint(constraint!((x + y) == 10.0));
        builder.add_constraint(constraint!((x) <= 5.0));

        assert_eq!(builder.constraints.len(), 2);
    }

    #[test]
    fn test_constraint_builders() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Test convenience builders
        let c = Constraint::eq(x + 5.0, 10.0);
        assert_eq!(c.sense, ConstraintSense::Equal);

        let c = Constraint::le(x * 2.0, 10.0);
        assert_eq!(c.sense, ConstraintSense::LessEqual);

        let c = Constraint::ge(x - 1.0, 0.0);
        assert_eq!(c.sense, ConstraintSense::GreaterEqual);

        let c = Constraint::gt(x, 0.0);
        assert_eq!(c.sense, ConstraintSense::Greater);
    }

    #[test]
    fn test_add_variable_to_linear_expression() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Build an expression like 2.0 * x + 5.0
        let expr = 2.0 * x + 5.0;

        // Verify the initial expression
        assert_eq!(expr.terms.len(), 1);
        assert_eq!(expr.terms[0].coefficient, 2.0);
        assert_eq!(expr.terms[0].variable, x);
        assert_eq!(expr.constant, 5.0);

        // Perform expr + y using the Add<VariableId> for LinearExpression implementation
        let result = expr + y;

        // Assert that the resulting LinearExpression has two terms and preserves the constant (5.0)
        assert_eq!(
            result.terms.len(),
            2,
            "Result should have exactly two terms"
        );
        assert_eq!(result.constant, 5.0, "Constant should be preserved as 5.0");

        // Check first term (should be 2.0 * x)
        assert_eq!(result.terms[0].coefficient, 2.0);
        assert_eq!(result.terms[0].variable, x);

        // Check second term (should be 1.0 * y, added from the VariableId)
        assert_eq!(result.terms[1].coefficient, 1.0);
        assert_eq!(result.terms[1].variable, y);
    }
}
