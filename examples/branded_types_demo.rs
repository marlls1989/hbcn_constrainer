use hbcn::constraint;
/// Example demonstrating type-safe variable binding with branded types
///
/// This example shows how the type system prevents mixing variables from
/// different LP model builders at compile time.
///
/// For full documentation on branded types, see the `lp_solver` module documentation:
/// Run `cargo doc --open` and navigate to the `lp_solver` module.
use hbcn::lp_model_builder;
use hbcn::lp_solver::{LPModelBuilder, OptimisationSense, VariableType};

fn main() {
    println!("=== Branded Types Type Safety Demo ===\n");

    // Example 1: Using the default brand for simple models
    println!("Example 1: Default brand (most common case)");
    example_default_brand();

    // Example 2: Using custom brands for type safety
    println!("\nExample 2: Custom brands for type safety");
    example_custom_brands();
}

/// Simple model using the macro for unique branding
fn example_default_brand() {
    let mut builder = lp_model_builder!();

    let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
    let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

    // Build expressions and constraints naturally
    builder.add_constraint(constraint!((x + y) <= 10.0));
    builder.add_constraint(constraint!((x) >= 0.0));
    builder.add_constraint(constraint!((y) >= 0.0));

    // Set objective and solve
    builder.set_objective(2.0 * x + 3.0 * y, OptimisationSense::Maximise);

    match builder.solve() {
        Ok(solution) => {
            println!("  Optimal solution found!");
            println!("  Objective value: {:.2}", solution.objective_value);
            if let Some(x_val) = solution.get_value(x) {
                println!("  x = {:.2}", x_val);
            }
            if let Some(y_val) = solution.get_value(y) {
                println!("  y = {:.2}", y_val);
            }
        }
        Err(e) => println!("  Error solving: {}", e),
    }
}

/// Example with custom brands showing type safety
fn example_custom_brands() {
    // Define unique types as brands
    struct ProductionModel;
    struct TransportModel;

    // Create two separate models with different brands
    let mut production: LPModelBuilder<ProductionModel> = LPModelBuilder::new();
    let mut transport: LPModelBuilder<TransportModel> = LPModelBuilder::new();

    // Variables from production model
    let prod_a = production.add_variable(VariableType::Continuous, 0.0, 100.0);
    let prod_b = production.add_variable(VariableType::Continuous, 0.0, 100.0);

    // Variables from transport model
    let truck_1 = transport.add_variable(VariableType::Integer, 0.0, 10.0);
    let truck_2 = transport.add_variable(VariableType::Integer, 0.0, 10.0);

    // These work fine - variables match their builder's brand
    production.add_constraint(constraint!((prod_a + prod_b) <= 100.0));
    transport.add_constraint(constraint!((truck_1 + truck_2) >= 3.0));

    println!("  ✓ Production model has {} variables", 2);
    println!("  ✓ Transport model has {} variables", 2);
    println!("  ✓ Type system prevents mixing variables between models!");

    // The following would NOT compile (uncomment to verify):

    // ERROR: Cannot use production variables in transport model
    // transport.add_constraint(constraint!((prod_a) <= 50.0));
    //   ^^^ expected `TransportModel`, found `ProductionModel`

    // ERROR: Cannot use transport variables in production model
    // production.add_constraint(constraint!((truck_1) >= 1.0));
    //   ^^^ expected `ProductionModel`, found `TransportModel`

    // ERROR: Cannot mix variables from different models
    // let mixed = prod_a + truck_1;
    //   ^^^ expected `ProductionModel`, found `TransportModel`

    println!("\n  This compile-time safety prevents common bugs like:");
    println!("  - Accidentally using variables from the wrong model");
    println!("  - Mixing constraints meant for different optimisation problems");
    println!("  - Reusing variable IDs incorrectly across models");
}
