//! Linear Programming (LP) solver abstraction layer
//! 
//! This module provides a trait-based abstraction for LP solvers, allowing the codebase
//! to be independent of specific solver implementations like Gurobi and coin_cbc.

use anyhow::Result;
use std::env;

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
pub enum SolverBackend {
    /// Gurobi commercial solver
    Gurobi,
    /// Coin CBC open-source solver
    CoinCbc,
}

impl SolverBackend {
    /// Get the solver backend from environment variable or use fallback logic
    pub fn from_env_or_default() -> Result<Self> {
        // Check if HBCN_LP_SOLVER environment variable is set
        if let Ok(solver_name) = env::var("HBCN_LP_SOLVER") {
            match solver_name.to_lowercase().as_str() {
                "gurobi" => {
                    #[cfg(feature = "gurobi")]
                    return Ok(SolverBackend::Gurobi);
                    #[cfg(not(feature = "gurobi"))]
                    return Err(anyhow::anyhow!("Gurobi solver requested via HBCN_LP_SOLVER but gurobi feature not enabled"));
                }
                "coin_cbc" | "coin-cbc" | "cbc" => {
                    #[cfg(feature = "coin_cbc")]
                    return Ok(SolverBackend::CoinCbc);
                    #[cfg(not(feature = "coin_cbc"))]
                    return Err(anyhow::anyhow!("Coin CBC solver requested via HBCN_LP_SOLVER but coin_cbc feature not enabled"));
                }
                _ => {
                    return Err(anyhow::anyhow!("Invalid solver '{}' in HBCN_LP_SOLVER. Valid options: gurobi, coin_cbc", solver_name));
                }
            }
        }
        
        // Fallback logic: prefer gurobi if available, then coin_cbc
        #[cfg(all(feature = "gurobi", not(feature = "coin_cbc")))]
        return Ok(SolverBackend::Gurobi);
        
        #[cfg(all(feature = "coin_cbc", not(feature = "gurobi")))]
        return Ok(SolverBackend::CoinCbc);
        
        #[cfg(all(feature = "gurobi", feature = "coin_cbc"))]
        return Ok(SolverBackend::Gurobi); // Prefer gurobi when both are available
        
        #[cfg(not(any(feature = "gurobi", feature = "coin_cbc")))]
        Err(anyhow::anyhow!("No LP solver backend available. Please enable a solver feature (e.g., 'gurobi' or 'coin_cbc')"))
    }
}

/// A linear expression term: coefficient * variable
#[derive(Debug, Clone)]
pub struct LinearTerm {
    pub coefficient: f64,
    pub variable: VariableId,
}

/// A linear expression: sum of terms plus constant
#[derive(Debug, Clone)]
pub struct LinearExpression {
    pub terms: Vec<LinearTerm>,
    pub constant: f64,
}

impl LinearExpression {
    /// Create a new linear expression with a constant term
    pub fn new(constant: f64) -> Self {
        Self {
            terms: Vec::new(),
            constant,
        }
    }

    /// Add a term to the expression
    pub fn add_term(&mut self, coefficient: f64, variable: VariableId) {
        self.terms.push(LinearTerm {
            coefficient,
            variable,
        });
    }

    /// Create a linear expression from a single variable
    pub fn from_variable(variable: VariableId) -> Self {
        Self {
            terms: vec![LinearTerm {
                coefficient: 1.0,
                variable,
            }],
            constant: 0.0,
        }
    }
}

impl std::ops::Mul<f64> for LinearExpression {
    type Output = LinearExpression;

    fn mul(mut self, coefficient: f64) -> Self::Output {
        for term in &mut self.terms {
            term.coefficient *= coefficient;
        }
        self.constant *= coefficient;
        self
    }
}

impl std::ops::Add<LinearExpression> for LinearExpression {
    type Output = LinearExpression;

    fn add(mut self, mut other: LinearExpression) -> Self::Output {
        self.terms.append(&mut other.terms);
        self.constant += other.constant;
        self
    }
}

impl std::ops::Sub<LinearExpression> for LinearExpression {
    type Output = LinearExpression;

    fn sub(mut self, other: LinearExpression) -> Self::Output {
        let mut negated = other * -1.0;
        self.terms.append(&mut negated.terms);
        self.constant += negated.constant;
        self
    }
}

/// Unique identifier for a variable in the LP model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId(usize);

/// Unique identifier for a constraint in the LP model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstraintId(usize);

/// Result of solving an LP model
#[derive(Debug, Clone)]
pub struct LPSolution {
    pub status: OptimizationStatus,
    pub objective_value: f64,
    pub variable_values: std::collections::HashMap<VariableId, f64>,
}

/// Builder for LP models that can work with different backends
pub struct LPModelBuilder {
    variables: std::collections::HashMap<VariableId, (String, VariableType, f64, f64)>,
    constraints: Vec<(String, LinearExpression, ConstraintSense, f64)>,
    objective: Option<(LinearExpression, OptimizationSense)>,
    next_var_id: usize,
    next_constr_id: usize,
}

impl LPModelBuilder {
    /// Create a new LP model builder
    pub fn new() -> Self {
        Self {
            variables: std::collections::HashMap::new(),
            constraints: Vec::new(),
            objective: None,
            next_var_id: 0,
            next_constr_id: 0,
        }
    }

    /// Add a variable to the model
    pub fn add_variable(
        &mut self,
        name: &str,
        var_type: VariableType,
        lower_bound: f64,
        upper_bound: f64,
    ) -> VariableId {
        let var_id = VariableId(self.next_var_id);
        self.next_var_id += 1;
        self.variables.insert(var_id, (name.to_string(), var_type, lower_bound, upper_bound));
        var_id
    }

    /// Add a constraint to the model
    pub fn add_constraint(
        &mut self,
        name: &str,
        expression: LinearExpression,
        sense: ConstraintSense,
        rhs: f64,
    ) -> ConstraintId {
        let constr_id = ConstraintId(self.next_constr_id);
        self.next_constr_id += 1;
        self.constraints.push((name.to_string(), expression, sense, rhs));
        constr_id
    }

    /// Set the objective function
    pub fn set_objective(&mut self, expression: LinearExpression, sense: OptimizationSense) {
        self.objective = Some((expression, sense));
    }

    /// Solve the model using the specified solver
    pub fn solve(self) -> Result<LPSolution> {
        let solver = SolverBackend::from_env_or_default()?;
        
        match solver {
            #[cfg(feature = "gurobi")]
            SolverBackend::Gurobi => crate::lp_solver::gurobi::solve_gurobi(self),
            #[cfg(not(feature = "gurobi"))]
            SolverBackend::Gurobi => Err(anyhow::anyhow!("Gurobi solver selected but gurobi feature not enabled")),
            
            #[cfg(feature = "coin_cbc")]
            SolverBackend::CoinCbc => crate::lp_solver::coin_cbc::solve_coin_cbc(self),
            #[cfg(not(feature = "coin_cbc"))]
            SolverBackend::CoinCbc => Err(anyhow::anyhow!("Coin CBC solver selected but coin_cbc feature not enabled")),
        }
    }
}

impl Default for LPModelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy trait for backward compatibility (deprecated)
#[allow(dead_code)]
pub trait LPSolver {
    /// Create a new LP model
    fn new_model(&self, name: &str) -> Result<Box<dyn LPModel>>;
}

/// Legacy trait for backward compatibility (deprecated)
#[allow(dead_code)]
pub trait LPModel {
    /// Add a variable to the model
    fn add_variable(
        &mut self,
        name: &str,
        var_type: VariableType,
        lower_bound: f64,
        upper_bound: f64,
    ) -> Result<VariableId>;

    /// Add a constraint to the model
    fn add_constraint(
        &mut self,
        name: &str,
        expression: LinearExpression,
        sense: ConstraintSense,
        rhs: f64,
    ) -> Result<ConstraintId>;

    /// Set the objective function
    fn set_objective(&mut self, expression: LinearExpression, sense: OptimizationSense) -> Result<()>;

    /// Update the model (prepare for solving)
    fn update(&mut self) -> Result<()>;

    /// Solve the optimization problem
    fn optimize(&mut self) -> Result<()>;

    /// Get the optimization status
    fn status(&self) -> Result<OptimizationStatus>;

    /// Get the value of a variable in the solution
    fn get_variable_value(&self, variable: VariableId) -> Result<f64>;

    /// Get the objective value
    fn get_objective_value(&self) -> Result<f64>;

    /// Get the number of variables in the model
    fn num_variables(&self) -> usize;

    /// Get the number of constraints in the model
    fn num_constraints(&self) -> usize;
}

/// Factory function to create an LP model (legacy - use LPModelBuilder instead)
#[allow(dead_code, unused_variables)]
pub fn create_lp_model(name: &str) -> Result<Box<dyn LPModel>> {
    #[cfg(feature = "gurobi")]
    {
        let solver = crate::lp_solver::gurobi::GurobiSolver;
        return solver.new_model(name);
    }
    
    #[cfg(feature = "coin_cbc")]
    {
        // coin_cbc doesn't have a legacy solver interface, use LPModelBuilder instead
        Err(anyhow::anyhow!("Legacy solver interface not available for coin_cbc. Use LPModelBuilder instead."))
    }
    
    #[cfg(not(any(feature = "gurobi", feature = "coin_cbc")))]
    {
        Err(anyhow::anyhow!("No LP solver backend available. Please enable a solver feature (e.g., 'gurobi' or 'coin_cbc')"))
    }
}

#[cfg(feature = "gurobi")]
pub mod gurobi;


#[cfg(feature = "coin_cbc")]
pub mod coin_cbc;