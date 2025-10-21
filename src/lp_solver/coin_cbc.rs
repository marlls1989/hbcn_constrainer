use std::collections::HashMap;

use crate::lp_solver::*;
use ::coin_cbc::{Model, Sense};

/// Round a floating-point number to 6 significant digits
fn round_to_6_sig_digits(value: f64) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    
    let magnitude = value.abs().log10().floor() as i32;
    let scale = 10_f64.powi(5 - magnitude);
    (value * scale).round() / scale
}

/// Solve an LP model using Coin CBC
pub fn solve_coin_cbc(builder: LPModelBuilder) -> Result<LPSolution> {
    let mut model = Model::default();
    let mut var_map = HashMap::new();
    
    // Add variables to the model
    for (var_id, (name, var_type, lower_bound, upper_bound)) in &builder.variables {
        let col = match var_type {
            VariableType::Continuous => {
                let col = model.add_col();
                model.set_col_lower(col, *lower_bound);
                model.set_col_upper(col, *upper_bound);
                col
            }
            VariableType::Integer => {
                let col = model.add_integer();
                model.set_col_lower(col, *lower_bound);
                model.set_col_upper(col, *upper_bound);
                col
            }
            VariableType::Binary => {
                model.add_binary()
            }
        };
        var_map.insert(*var_id, col);
    }
    
    // Add constraints
    for (_, expression, sense, rhs) in &builder.constraints {
        let row = model.add_row();
        
        for term in &expression.terms {
            if let Some(&col) = var_map.get(&term.variable) {
                model.set_weight(row, col, term.coefficient);
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        
        // Handle constant term
        let rhs_adjusted = rhs - expression.constant;
        
        // Add constraint based on sense
        match sense {
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
    if let Some((expression, sense)) = &builder.objective {
        for term in &expression.terms {
            if let Some(&col) = var_map.get(&term.variable) {
                model.set_obj_coeff(col, term.coefficient);
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        
        let sense = match sense {
            OptimizationSense::Minimize => Sense::Minimize,
            OptimizationSense::Maximize => Sense::Maximize,
        };
        
        model.set_obj_sense(sense);
    }
    
    // Solve the model
    let solution = model.solve();
    
    // Extract variable values from solution
    let mut variable_values = HashMap::new();
    for (var_id, col) in var_map.iter() {
        let value = round_to_6_sig_digits(solution.col(*col));
        variable_values.insert(*var_id, value);
    }
    
    // Calculate objective value
    let objective_value = if let Some((expression, _)) = &builder.objective {
        let mut obj_val = expression.constant;
        for term in &expression.terms {
            if let Some(&value) = variable_values.get(&term.variable) {
                obj_val += term.coefficient * value;
            }
        }
        round_to_6_sig_digits(obj_val)
    } else {
        0.0
    };
    
    // Determine optimization status
    let status = if solution.raw().is_proven_optimal() {
        OptimizationStatus::Optimal
    } else if solution.raw().is_proven_infeasible() {
        OptimizationStatus::Infeasible
    } else {
        OptimizationStatus::Other("Unknown status")
    };
    
    Ok(LPSolution {
        status,
        objective_value,
        variable_values,
    })
}
