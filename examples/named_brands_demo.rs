//! Demo of the new named brand feature for lp_model_builder! macro
//!
//! This example demonstrates how the optional brand name parameter makes it easier
//! to identify different models in the type system and debugging.

use hbcn::{constraint, lp_model_builder};
use hbcn::lp_solver::{VariableType, OptimizationSense};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Named Brand Demo ===\n");

    // Create two models with descriptive brand names
    let mut production_model = lp_model_builder!(ProductionModel);
    let mut logistics_model = lp_model_builder!(LogisticsModel);

    println!("1. Created two models with named brands:");
    println!("   - ProductionModel");
    println!("   - LogisticsModel\n");

    // Add variables to each model
    let widgets = production_model.add_variable("widgets", VariableType::Continuous, 0.0, 1000.0);
    let gadgets = production_model.add_variable("gadgets", VariableType::Continuous, 0.0, 500.0);

    let trucks = logistics_model.add_variable("trucks", VariableType::Integer, 0.0, 10.0);
    let routes = logistics_model.add_variable("routes", VariableType::Integer, 0.0, 20.0);

    println!("2. Added variables to each model:");
    println!("   Production: widgets, gadgets");
    println!("   Logistics: trucks, routes\n");

    // Add constraints using the constraint! macro
    production_model.add_constraint(constraint!("production_capacity", (widgets + 2.0 * gadgets) <= 1200.0));
    production_model.add_constraint(constraint!("labor_hours", (0.5 * widgets + gadgets) <= 400.0));
    
    logistics_model.add_constraint(constraint!("fleet_size", (trucks) <= 8.0));
    logistics_model.add_constraint(constraint!("route_capacity", (routes - 3.0 * trucks) <= 0.0));

    println!("3. Added constraints with descriptive names:");
    println!("   Production: production_capacity, labor_hours");
    println!("   Logistics: fleet_size, route_capacity\n");

    // Set objectives
    production_model.set_objective(50.0 * widgets + 80.0 * gadgets, OptimizationSense::Maximize);
    logistics_model.set_objective(trucks * 100.0 + routes * 20.0, OptimizationSense::Minimize);

    println!("4. Set different objectives:");
    println!("   Production: Maximize profit (50*widgets + 80*gadgets)");
    println!("   Logistics: Minimize cost (100*trucks + 20*routes)\n");

    // The following would cause compile-time errors due to different brands:
    // production_model.add_constraint(constraint!((trucks) <= 5.0));  // ERROR!
    // let mixed = widgets + trucks;  // ERROR!
    
    println!("✅ Type safety enforced: Variables from different models cannot be mixed!");
    println!("   This prevents accidental bugs when working with multiple models.\n");

    // Anonymous brands still work as before
    let mut anonymous_model = lp_model_builder!();
    let _temp_var = anonymous_model.add_variable("temp", VariableType::Continuous, 0.0, 100.0);
    
    println!("5. Anonymous brands still work:");
    println!("   Created anonymous model with temp variable\n");

    // Show that each anonymous call creates a unique brand
    let mut another_anonymous = lp_model_builder!();
    let _another_var = another_anonymous.add_variable("another", VariableType::Continuous, 0.0, 100.0);
    
    // This would also cause a compile error:
    // let mixed_anonymous = temp_var + another_var;  // ERROR!
    
    println!("✅ Each anonymous brand is also unique and type-safe!\n");

    println!("=== Benefits of Named Brands ===");
    println!("• Better error messages (shows 'ProductionModel' vs 'LogisticsModel')");
    println!("• Easier debugging and code documentation");
    println!("• Same compile-time type safety as before");  
    println!("• Backward compatible - anonymous brands still work");
    println!("• Zero runtime overhead - brands are phantom types\n");

    Ok(())
}
