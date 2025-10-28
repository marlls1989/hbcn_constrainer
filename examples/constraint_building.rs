//! Example demonstrating constraint building with the LP solver API

use hbcn::constraint;
use hbcn::lp_model_builder;
use hbcn::lp_solver::{OptimizationSense, VariableType};

fn main() -> anyhow::Result<()> {
    let mut builder = lp_model_builder!();

    // Create variables
    let x = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);
    let y = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);

    // Method 1: Use the constraint! macro (unnamed - most concise)
    println!("Using constraint! macro (unnamed):");
    builder.add_constraint(constraint!((2.0 * x + 3.0 * y) <= 100.0));
    builder.add_constraint(constraint!((x - y) >= 5.0));
    builder.add_constraint(constraint!((x + y) == 50.0));
    builder.add_constraint(constraint!((x) > 0.0));

    // Alternative: Named constraints for debugging
    // builder.add_constraint(constraint!("important", (x + y) == 50.0));

    // Method 2: Use Constraint builder methods
    // builder.add_constraint(Constraint::le(2.0 * x + 3.0 * y, 100.0));  // unnamed
    // builder.add_constraint(Constraint::le_named("c1", 2.0 * x + 3.0 * y, 100.0));  // named

    // Set objective: maximize x + 2y
    builder.set_objective(x + 2.0 * y, OptimizationSense::Maximize);

    // Solve
    let solution = builder.solve()?;
    println!("\nSolution:");
    println!("Status: {:?}", solution.status);
    println!("Objective value: {}", solution.objective_value);
    println!("x = {}", solution.get_value(x).unwrap());
    println!("y = {}", solution.get_value(y).unwrap());

    // Verify constraints
    let x_val = solution.get_value(x).unwrap();
    let y_val = solution.get_value(y).unwrap();
    println!("\nConstraint verification:");
    println!("2x + 3y = {} (should be <= 100)", 2.0 * x_val + 3.0 * y_val);
    println!("x - y = {} (should be >= 5)", x_val - y_val);
    println!("x + y = {} (should be == 50)", x_val + y_val);

    Ok(())
}
