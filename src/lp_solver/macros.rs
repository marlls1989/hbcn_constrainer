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
/// let mut builder = lp_model_builder!();
/// let x = builder.add_variable("x", VariableType::Continuous, 0.0, 10.0);
/// // Each call to lp_model_builder!() creates a unique brand
/// ```
#[macro_export]
macro_rules! lp_model_builder {
    () => {{
        // Create a unique brand type for each macro invocation
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
/// let mut builder = lp_model_builder!();
/// let x = builder.add_variable("x", VariableType::Continuous, 0.0, 10.0);
/// let y = builder.add_variable("y", VariableType::Continuous, 0.0, 10.0);
///
/// // Unnamed constraints
/// let c1 = constraint!((x + y) == 10.0);
/// let c2 = constraint!((2.0 * x) <= 5.0);
/// let c3 = constraint!((x - y) >= 0.0);
/// let c4 = constraint!((x) > 1.0);
///
/// // Named constraints
/// let c5 = constraint!("my_constraint", (x + y) == 10.0);
/// builder.add_constraint(constraint!("my_constraint", (x + y) == 10.0));
/// ```
#[macro_export]
macro_rules! constraint {
    // Unnamed constraints (most common case)
    (($lhs:expr) == $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from(""),
            $lhs,
            $crate::lp_solver::ConstraintSense::Equal,
            $rhs as f64,
        )
    };
    (($lhs:expr) <= $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from(""),
            $lhs,
            $crate::lp_solver::ConstraintSense::LessEqual,
            $rhs as f64,
        )
    };
    (($lhs:expr) >= $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from(""),
            $lhs,
            $crate::lp_solver::ConstraintSense::GreaterEqual,
            $rhs as f64,
        )
    };
    (($lhs:expr) > $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from(""),
            $lhs,
            $crate::lp_solver::ConstraintSense::Greater,
            $rhs as f64,
        )
    };

    // Named constraints (with name parameter)
    ($name:expr, ($lhs:expr) == $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from($name),
            $lhs,
            $crate::lp_solver::ConstraintSense::Equal,
            $rhs as f64,
        )
    };
    ($name:expr, ($lhs:expr) <= $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from($name),
            $lhs,
            $crate::lp_solver::ConstraintSense::LessEqual,
            $rhs as f64,
        )
    };
    ($name:expr, ($lhs:expr) >= $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from($name),
            $lhs,
            $crate::lp_solver::ConstraintSense::GreaterEqual,
            $rhs as f64,
        )
    };
    ($name:expr, ($lhs:expr) > $rhs:expr) => {
        $crate::lp_solver::Constraint::new(
            std::sync::Arc::from($name),
            $lhs,
            $crate::lp_solver::ConstraintSense::Greater,
            $rhs as f64,
        )
    };
}
