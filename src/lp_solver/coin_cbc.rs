use std::collections::HashMap;

use crate::lp_solver::output_suppression::GagHandle;
use crate::lp_solver::*;
use ::coin_cbc::{Model, Sense};

/// Round a floating-point number to a specified number of significant digits
/// This is an workaround to mask floating point errors in CBC.
fn round_to_sig_digits(value: f64, digits: u32) -> f64 {
    if value == 0.0 {
        return 0.0;
    }

    let magnitude = value.abs().log10().floor() as i32;
    let scale = 10_f64.powi(digits as i32 - magnitude - 1);
    (value * scale).round() / scale
}

/// Solve an LP model using Coin CBC
pub fn solve_coin_cbc<Brand>(builder: &LPModelBuilder<Brand>) -> Result<LPSolution<Brand>> {
    // Redirect CBC's verbose output to lp_solver.log
    let _gag_handle = GagHandle::stdout()?;
    let mut model = Model::default();
    let mut var_map = HashMap::new();

    // Add variables to the model
    for (idx, var_info) in builder.variables.iter().enumerate() {
        let col = match var_info.var_type {
            VariableType::Continuous => {
                let col = model.add_col();
                model.set_col_lower(col, var_info.lower_bound);
                model.set_col_upper(col, var_info.upper_bound);
                col
            }
            VariableType::Integer => {
                let col = model.add_integer();
                model.set_col_lower(col, var_info.lower_bound);
                model.set_col_upper(col, var_info.upper_bound);
                col
            }
            VariableType::Binary => model.add_binary(),
        };
        let var_id = VariableId {
            id: idx,
            _brand: std::marker::PhantomData,
        };
        var_map.insert(var_id, col);
    }

    // Add constraints
    for constraint in &builder.constraints {
        let row = model.add_row();

        for term in &constraint.expression.terms {
            if let Some(&col) = var_map.get(&term.variable) {
                model.set_weight(row, col, term.coefficient);
            } else {
                return Err(anyhow::anyhow!(
                    "Variable {:?} not found in model",
                    term.variable
                ));
            }
        }

        // Handle constant term
        let rhs_adjusted = constraint.rhs - constraint.expression.constant;

        // Add constraint based on sense
        match constraint.sense {
            ConstraintSense::LessEqual => {
                model.set_row_upper(row, rhs_adjusted);
            }
            ConstraintSense::Equal => {
                model.set_row_equal(row, rhs_adjusted);
            }
            ConstraintSense::GreaterEqual => {
                model.set_row_lower(row, rhs_adjusted);
            }
            ConstraintSense::Greater => {
                // Coin CBC doesn't support strict inequalities, use >= with small epsilon
                model.set_row_lower(row, rhs_adjusted + 1e-10);
            }
        }
    }

    // Set objective function
    if let Some(obj_info) = &builder.objective {
        for term in &obj_info.expression.terms {
            if let Some(&col) = var_map.get(&term.variable) {
                model.set_obj_coeff(col, term.coefficient);
            } else {
                return Err(anyhow::anyhow!(
                    "Variable {:?} not found in model",
                    term.variable
                ));
            }
        }

        let sense = match obj_info.sense {
            OptimisationSense::Minimise => Sense::Minimize,
            OptimisationSense::Maximise => Sense::Maximize,
        };

        model.set_obj_sense(sense);
    }

    // Solve the model
    let solution = model.solve();

    // Extract variable values from solution
    let num_vars = builder.variables.len();
    let mut variable_values = vec![0.0; num_vars];
    for (var_id, col) in var_map.iter() {
        let value = round_to_sig_digits(solution.col(*col), 8);
        variable_values[var_id.id] = value;
    }

    // Calculate objective value
    let objective_value = if let Some(obj_info) = &builder.objective {
        let mut obj_val = obj_info.expression.constant;
        for term in &obj_info.expression.terms {
            let value = variable_values[term.variable.id];
            obj_val += term.coefficient * value;
        }
        round_to_sig_digits(obj_val, 8)
    } else {
        0.0
    };

        // Determine optimisation status
    let status = if solution.raw().is_proven_optimal() {
        OptimisationStatus::Optimal
    } else if solution.raw().is_proven_infeasible() {
        OptimisationStatus::Infeasible
    } else {
        OptimisationStatus::Other("Unknown status")
    };

    Ok(LPSolution {
        status,
        objective_value,
        variable_values,
        _brand: std::marker::PhantomData,
    })
}
