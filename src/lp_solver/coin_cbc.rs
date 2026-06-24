use std::collections::HashMap;

use crate::lp_solver::*;
use crate::output_suppression::GagHandle;
use ::coin_cbc::{Model, Sense};

/// Round a floating-point number to a specified number of significant digits
/// This is an workaround to mask floating point errors in CBC.
fn round_to_sig_digits(value: f64, digits: u32) -> f64 {
    // Leave zero and non-finite values (NaN / infinity) untouched: log10 of these
    // would produce garbage that propagates into delay/objective numbers.
    if value == 0.0 || !value.is_finite() {
        return value;
    }

    let magnitude = value.abs().log10().floor() as i32;
    let scale = 10_f64.powi(digits as i32 - magnitude - 1);
    (value * scale).round() / scale
}

/// Solve an LP model using Coin CBC
pub fn solve_coin_cbc<Brand>(builder: &LPModelBuilder<Brand>) -> Result<LPSolution<Brand>> {
    // Redirect CBC's verbose output to hbcn.log (unless in verbose mode)
    let _gag_handle = GagHandle::stdout();
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

        // Coalesce duplicate variables before handing off: CBC's `set_weight` OVERWRITES
        // the coefficient for a (row, col) pair, so a variable appearing more than once in
        // one expression would silently collapse to its last coefficient instead of summing
        // (Gurobi's `add_term` sums). Accumulate here so both backends agree.
        let coalesced = coalesce_terms(&constraint.expression.terms);
        for (variable, coefficient) in coalesced {
            if let Some(&col) = var_map.get(&variable) {
                model.set_weight(row, col, coefficient);
            } else {
                return Err(anyhow::anyhow!(
                    "Variable {:?} not found in model",
                    variable
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
                // Coin CBC doesn't support strict inequalities, use >= with a small epsilon.
                // Scale the epsilon to the RHS magnitude so it is not lost to float precision
                // when the RHS is large (a fixed 1e-10 added to a large value rounds away,
                // silently degrading `>` to `>=`).
                let epsilon = 1e-10 * rhs_adjusted.abs().max(1.0);
                model.set_row_lower(row, rhs_adjusted + epsilon);
            }
        }
    }

    // Set objective function (coalesce duplicate variables, as with constraint rows).
    if let Some(obj_info) = &builder.objective {
        for (variable, coefficient) in coalesce_terms(&obj_info.expression.terms) {
            if let Some(&col) = var_map.get(&variable) {
                model.set_obj_coeff(col, coefficient);
            } else {
                return Err(anyhow::anyhow!(
                    "Variable {:?} not found in model",
                    variable
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

    // Determine optimisation status BEFORE reading any values.
    let status = if solution.raw().is_proven_optimal() {
        OptimisationStatus::Optimal
    } else if solution.raw().is_proven_infeasible() {
        OptimisationStatus::Infeasible
    } else {
        OptimisationStatus::Other("Unknown status")
    };

    // Only extract column/objective values when the solve actually succeeded. For an
    // infeasible/unknown solve CBC's post-solve columns are meaningless; reading them
    // (as we used to, unconditionally) would hand callers garbage delays. Mirror the
    // Gurobi backend, which returns zeros for any non-optimal status.
    let num_vars = builder.variables.len();
    let mut variable_values = vec![0.0; num_vars];
    let objective_value = if status == OptimisationStatus::Optimal {
        for (var_id, col) in var_map.iter() {
            variable_values[var_id.id] = round_to_sig_digits(solution.col(*col), 8);
        }

        if let Some(obj_info) = &builder.objective {
            let mut obj_val = obj_info.expression.constant;
            for term in &obj_info.expression.terms {
                obj_val += term.coefficient * variable_values[term.variable.id];
            }
            round_to_sig_digits(obj_val, 8)
        } else {
            0.0
        }
    } else {
        0.0
    };

    Ok(LPSolution {
        status,
        objective_value,
        variable_values,
        _brand: std::marker::PhantomData,
    })
}

/// Sum coefficients of repeated variables so each variable is handed to CBC exactly once.
///
/// Insertion order of first appearance is preserved for deterministic model construction.
fn coalesce_terms<Brand>(terms: &[LinearTerm<Brand>]) -> Vec<(VariableId<Brand>, f64)> {
    let mut order: Vec<VariableId<Brand>> = Vec::new();
    let mut sums: HashMap<VariableId<Brand>, f64> = HashMap::new();
    for term in terms {
        if let std::collections::hash_map::Entry::Vacant(e) = sums.entry(term.variable) {
            e.insert(term.coefficient);
            order.push(term.variable);
        } else {
            *sums.get_mut(&term.variable).unwrap() += term.coefficient;
        }
    }
    order.into_iter().map(|v| (v, sums[&v])).collect()
}
