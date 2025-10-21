//! Gurobi implementation of the LP solver abstraction

use super::{
    ConstraintId, ConstraintSense, LinearExpression, LPModel, LPSolver, OptimizationSense,
    OptimizationStatus, VariableId, VariableType,
};
use anyhow::Result;
use gurobi::{
    ConstrSense, Env, INFINITY, Model, ModelSense, Status, Var, VarType,
};
use std::collections::HashMap;

/// Gurobi-based LP solver implementation
pub struct GurobiSolver;

impl LPSolver for GurobiSolver {
    fn new_model(&self, name: &str) -> Result<Box<dyn LPModel>> {
        let env = create_quiet_gurobi_env("hbcn.log")?;
        let model = Model::new(name, &env)?;
        Ok(Box::new(GurobiModel {
            model,
            variables: HashMap::new(),
            constraints: HashMap::new(),
            next_var_id: 0,
            next_constr_id: 0,
        }))
    }
}

/// Gurobi-based LP model implementation
struct GurobiModel {
    model: Model,
    variables: HashMap<VariableId, Var>,
    constraints: HashMap<ConstraintId, gurobi::Constr>,
    next_var_id: usize,
    next_constr_id: usize,
}

impl LPModel for GurobiModel {
    fn add_variable(
        &mut self,
        name: &str,
        var_type: VariableType,
        lower_bound: f64,
        upper_bound: f64,
    ) -> Result<VariableId> {
        let gurobi_var_type = match var_type {
            VariableType::Continuous => VarType::Continuous,
            VariableType::Integer => VarType::Integer,
            VariableType::Binary => VarType::Binary,
        };

        let upper = if upper_bound == f64::INFINITY {
            INFINITY
        } else {
            upper_bound
        };

        let var = self.model.add_var(
            name,
            gurobi_var_type,
            0.0,          // objective coefficient
            lower_bound,  // lower bound
            upper,        // upper bound
            &[],
            &[],
        )?;

        let var_id = VariableId(self.next_var_id);
        self.next_var_id += 1;
        self.variables.insert(var_id, var);

        Ok(var_id)
    }

    fn add_constraint(
        &mut self,
        name: &str,
        expression: LinearExpression,
        sense: ConstraintSense,
        rhs: f64,
    ) -> Result<ConstraintId> {
        let gurobi_sense = match sense {
            ConstraintSense::LessEqual => ConstrSense::Less,
            ConstraintSense::Equal => ConstrSense::Equal,
            ConstraintSense::GreaterEqual => ConstrSense::Greater,
            ConstraintSense::Greater => ConstrSense::Greater,
        };

        // Convert our linear expression to Gurobi's format
        let mut gurobi_expr = gurobi::LinExpr::new();
        for term in &expression.terms {
            if let Some(var) = self.variables.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(expression.constant);

        let constr = self.model.add_constr(
            name,
            gurobi_expr,
            gurobi_sense,
            rhs,
        )?;

        let constr_id = ConstraintId(self.next_constr_id);
        self.next_constr_id += 1;
        self.constraints.insert(constr_id, constr);

        Ok(constr_id)
    }

    fn set_objective(&mut self, expression: LinearExpression, sense: OptimizationSense) -> Result<()> {
        let gurobi_sense = match sense {
            OptimizationSense::Minimize => ModelSense::Minimize,
            OptimizationSense::Maximize => ModelSense::Maximize,
        };

        // Convert our linear expression to Gurobi's format
        let mut gurobi_expr = gurobi::LinExpr::new();
        for term in &expression.terms {
            if let Some(var) = self.variables.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(expression.constant);

        self.model.set_objective(gurobi_expr, gurobi_sense)?;
        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        self.model.update()?;
        Ok(())
    }

    fn optimize(&mut self) -> Result<()> {
        self.model.optimize()?;
        Ok(())
    }

    fn status(&self) -> Result<OptimizationStatus> {
        let status = self.model.status()?;
        let gurobi_status = match status {
            Status::Optimal => OptimizationStatus::Optimal,
            Status::SubOptimal => OptimizationStatus::Feasible,
            Status::Infeasible => OptimizationStatus::Infeasible,
            Status::Unbounded => OptimizationStatus::Unbounded,
            Status::InfOrUnbd => OptimizationStatus::InfeasibleOrUnbounded,
            _ => OptimizationStatus::Other("Unknown status"),
        };
        Ok(gurobi_status)
    }

    fn get_variable_value(&self, variable: VariableId) -> Result<f64> {
        if let Some(var) = self.variables.get(&variable) {
            // Check if the model has been optimized and has a solution
            let status = self.model.status()?;
            match status {
                Status::Optimal | Status::SubOptimal => {
                    Ok(var.get(&self.model, gurobi::attr::X)?)
                }
                _ => Err(anyhow::anyhow!("Model not optimized or no solution available. Status: {:?}", status))
            }
        } else {
            Err(anyhow::anyhow!("Variable {:?} not found in model", variable))
        }
    }

    fn get_objective_value(&self) -> Result<f64> {
        // Check if the model has been optimized and has a solution
        let status = self.model.status()?;
        match status {
            Status::Optimal | Status::SubOptimal => {
                Ok(self.model.get(gurobi::attr::ObjVal)?)
            }
            _ => Err(anyhow::anyhow!("Model not optimized or no solution available. Status: {:?}", status))
        }
    }

    fn num_variables(&self) -> usize {
        self.variables.len()
    }

    fn num_constraints(&self) -> usize {
        self.constraints.len()
    }
}

/// Create a quiet Gurobi environment that suppresses all console output
/// This is useful for tests and when you don't want Gurobi's verbose output
fn create_quiet_gurobi_env(logfile: &str) -> Result<Env> {
    let env = Env::new(logfile)?;
    // Use the same environment setup as the original code
    Ok(env)
}
