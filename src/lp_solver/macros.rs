//! Macros for the LP solver module
//!
//! This module contains all the macros used by the LP solver, providing
//! convenient syntax for creating models and constraints.

/// Create a new LP model builder with a unique brand
///
/// This macro ensures that each model builder has a unique type-level brand,
/// preventing accidental mixing of variables between different models.
///
/// # Examples
///
/// ```rust
/// use hbcn::lp_model_builder;
/// use hbcn::lp_solver::VariableType;
///
/// // Anonymous brand (each call creates unique anonymous type)
/// let mut builder = lp_model_builder!();
/// let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
///
/// // Named brand (easier to identify in type system and errors)
/// let mut production_model = lp_model_builder!(ProductionModel);
/// let mut scheduling_model = lp_model_builder!(SchedulingModel);
///
/// let prod_var = production_model.add_variable(VariableType::Continuous, 0.0, 100.0);
/// let sched_var = scheduling_model.add_variable(VariableType::Continuous, 0.0, 24.0);
///
/// // This would cause a compile-time error due to different brands:
/// // scheduling_model.add_constraint(constraint!((prod_var) <= 50.0)); // ERROR!
/// ```
#[macro_export]
macro_rules! lp_model_builder {
    // Named brand - user provides the brand name
    ($brand_name:ident) => {{
        struct $brand_name;
        $crate::lp_solver::LPModelBuilder::<$brand_name>::new()
    }};

    // Anonymous brand - the `UniqueBrand` struct is defined locally within the `{{ ... }}` block,
    // so each macro invocation creates a fresh scope with its own distinct `UniqueBrand` type
    () => {{
        struct UniqueBrand;
        $crate::lp_solver::LPModelBuilder::<UniqueBrand>::new()
    }};
}

/// Create constraints using natural comparison syntax
///
/// This macro provides a declarative way to create `Constraint` objects using
/// comparison-like syntax. The left-hand side must be in parentheses.
///
/// # Examples
///
/// ```rust
/// use hbcn::constraint;
/// use hbcn::lp_model_builder;
/// use hbcn::lp_solver::VariableType;
///
/// // Use named brand for better type system clarity
/// let mut builder = lp_model_builder!(OptimisationModel);
/// let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
/// let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
///
/// // Unnamed constraints (most common)
/// let c1 = constraint!((x + y) == 10.0);
/// let c2 = constraint!((2.0 * x) <= 5.0);
/// let c3 = constraint!((x - y) >= 0.0);
/// let c4 = constraint!((x) > 1.0);
///
/// // Simple constraint creation
/// let c5 = constraint!((x + y) == 10.0);
/// builder.add_constraint(constraint!((2.0 * x) <= 15.0));
/// ```
#[macro_export]
macro_rules! constraint {
    (($lhs:expr) == $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            $lhs,
            $crate::lp_solver::ConstraintSense::Equal,
            $rhs as f64,
        )
    };
    (($lhs:expr) <= $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            $lhs,
            $crate::lp_solver::ConstraintSense::LessEqual,
            $rhs as f64,
        )
    };
    (($lhs:expr) >= $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            $lhs,
            $crate::lp_solver::ConstraintSense::GreaterEqual,
            $rhs as f64,
        )
    };
    (($lhs:expr) > $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            $lhs,
            $crate::lp_solver::ConstraintSense::Greater,
            $rhs as f64,
        )
    };
}

#[cfg(test)]
mod tests {
    use crate::lp_solver::VariableType;

    #[test]
    fn test_named_brand_lp_model_builder() {
        // Test that named brands work
        let mut model1 = lp_model_builder!(TestModel1);
        let mut model2 = lp_model_builder!(TestModel2);

        let x1 = model1.add_variable(VariableType::Continuous, 0.0, 10.0);
        let x2 = model2.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Variables should have different types due to different brands
        // This test just ensures the macro compiles and creates different types
        let _expr1 = x1 + 5.0;
        let _expr2 = x2 + 5.0;

        // This would NOT compile if uncommented (different brands):
        // let _mixed = x1 + x2; // ERROR: different brands
    }

    #[test]
    fn test_anonymous_brand_still_works() {
        // Test that anonymous brands still work as before
        let mut builder1 = lp_model_builder!();
        let mut builder2 = lp_model_builder!();

        let x = builder1.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = builder2.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Each anonymous brand should be unique
        let _expr1 = x + 1.0;
        let _expr2 = y + 2.0;

        // This would NOT compile if uncommented (different anonymous brands):
        // let _mixed = x + y; // ERROR: different brands
    }

    #[test]
    fn test_branded_constraints_work() {
        use crate::constraint;

        let mut model = lp_model_builder!(ConstraintTestModel);
        let x = model.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = model.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Test that constraints work with named brands
        let c1 = constraint!((x + y) == 10.0);
        let c2 = constraint!((x * 2.0) <= 20.0);

        model.add_constraint(c1);
        model.add_constraint(c2);

        // Should compile successfully
        assert_eq!(model.constraints.len(), 2);
    }
}
