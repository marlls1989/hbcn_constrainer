use std::collections::HashMap;

use ::gurobi::{ConstrSense, Env, LinExpr, Model, ModelSense, Status, VarType, attr};

use crate::lp_solver::output_suppression::GagHandle;
use crate::lp_solver::*;

/// Solve an LP model using Gurobi
pub fn solve_gurobi<Brand>(builder: &LPModelBuilder<Brand>) -> Result<LPSolution<Brand>> {
    // Redirect Gurobi's verbose output to lp_solver.log
    let _gag_handle = GagHandle::stdout()?;
    let env = Env::new("")?;
    let mut model = Model::new("lp_model", &env)?;

    // Add variables
    let mut var_map = HashMap::new();
    for (idx, var_info) in builder.variables.iter().enumerate() {
        let vtype = match var_info.var_type {
            VariableType::Continuous => VarType::Continuous,
            VariableType::Integer => VarType::Integer,
            VariableType::Binary => VarType::Binary,
        };

        let var = model.add_var(
            &format!("x_{}", idx),
            vtype,
            0.0, // objective coefficient
            var_info.lower_bound,
            var_info.upper_bound,
            &[], // coefficients for existing constraints
            &[], // constraint indices
        )?;

        let var_id = VariableId {
            id: idx,
            _brand: std::marker::PhantomData,
        };
        var_map.insert(var_id, var);
    }

    // Add constraints
    let mut constr_map = HashMap::new();
    for (constr_id, constraint) in builder.constraints.iter().enumerate() {
        let mut gurobi_expr = LinExpr::new();

        for term in &constraint.expression.terms {
            if let Some(var) = var_map.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!(
                    "Variable {:?} not found in model",
                    term.variable
                ));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(constraint.expression.constant);

        let sense = match constraint.sense {
            ConstraintSense::LessEqual => ConstrSense::Less,
            ConstraintSense::Equal => ConstrSense::Equal,
            ConstraintSense::GreaterEqual => ConstrSense::Greater,
            ConstraintSense::Greater => ConstrSense::Greater,
        };

        let constraint = model.add_constr(
            &format!("c_{}", constr_id),
            gurobi_expr,
            sense,
            constraint.rhs,
        )?;
        constr_map.insert(ConstraintId(constr_id), constraint);
    }

    // Update the model before setting objective
    model.update()?;

    // Set objective
    if let Some(obj_info) = &builder.objective {
        let mut gurobi_expr = LinExpr::new();

        for term in &obj_info.expression.terms {
            if let Some(var) = var_map.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!(
                    "Variable {:?} not found in model",
                    term.variable
                ));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(obj_info.expression.constant);

        let sense = match obj_info.sense {
            OptimisationSense::Minimise => ModelSense::Minimize,
            OptimisationSense::Maximise => ModelSense::Maximize,
        };

        model.set_objective(gurobi_expr, sense)?;
    }

    // Optimise
    model.optimize()?;

    // Get status
    let status = model.status()?;
    let optimisation_status = match status {
        Status::Optimal | Status::SubOptimal => OptimisationStatus::Optimal,
        Status::Infeasible => OptimisationStatus::Infeasible,
        Status::Unbounded => OptimisationStatus::Unbounded,
        _ => OptimisationStatus::Other("Unknown status"),
    };

    // Extract variable values and objective value only if model is feasible
    let num_vars = builder.variables.len();
    let mut variable_values = vec![0.0; num_vars];
    let objective_value = match optimisation_status {
        OptimisationStatus::Optimal => {
            // Get variable values
            for (var_id, var) in &var_map {
                let value = var.get(&model, attr::X)?;
                variable_values[var_id.id] = value;
            }

            // Get objective value
            model.get(attr::ObjVal)?
        }
        _ => {
            // For infeasible, unbounded, or other statuses, return default values
            0.0
        }
    };

    Ok(LPSolution {
        status: optimisation_status,
        objective_value,
        variable_values,
        _brand: std::marker::PhantomData,
    })
}
