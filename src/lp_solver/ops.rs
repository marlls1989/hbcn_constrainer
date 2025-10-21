//! Operator overloading for linear programming expressions
//!
//! This module provides convenient operator overloading for building linear expressions
//! using natural mathematical notation.
//!
//! # Expression Building
//!
//! Variables and expressions support natural arithmetic operators:
//!
//! ```ignore
//! let x = builder.add_variable("x", VariableType::Continuous, 0.0, 10.0);
//! let y = builder.add_variable("y", VariableType::Continuous, 0.0, 10.0);
//!
//! // All of these work naturally:
//! let expr1 = x + y;           // Addition
//! let expr2 = x - y;           // Subtraction  
//! let expr3 = 2.0 * x;         // Scalar multiplication (left)
//! let expr4 = x * 2.0;         // Scalar multiplication (right)
//! let expr5 = x + 2.0 * y + 5.0; // Complex expressions
//! let expr6 = (x + y) * 3.0;   // Parentheses work
//! ```
//!
//! # Constraint Macro
//!
//! The `constraint!` macro provides a declarative way to create constraints:
//!
//! ```ignore
//! // Unnamed constraints (most common)
//! let c1 = constraint!((x + y) == 10.0);
//! let c2 = constraint!((2.0 * x) <= 5.0);
//! let c3 = constraint!((x - y) >= 0.0);
//! let c4 = constraint!((x) > 1.0);
//!
//! // Named constraints for debugging
//! let c5 = constraint!("my_constraint", (x + y) == 10.0);
//! builder.add_constraint(constraint!("my_constraint", (x + y) == 10.0));
//! ```
//!
//! **Note:** The left-hand side must be in parentheses: `(expression) == value`
//!
//! # Type Safety
//!
//! All operations maintain the brand type parameter, ensuring variables from different
//! models cannot be accidentally mixed.

use super::{LinearExpression, LinearTerm, VariableId};

// ============================================================================
// Constraint Macro
// ============================================================================

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

// ============================================================================
// Operators for LinearExpression
// ============================================================================

impl<Brand> std::ops::Add<LinearExpression<Brand>> for LinearExpression<Brand> {
    type Output = LinearExpression<Brand>;

    fn add(self, other: LinearExpression<Brand>) -> Self::Output {
        let mut terms = self.terms;
        terms.extend(other.terms);
        LinearExpression {
            terms,
            constant: self.constant + other.constant,
        }
    }
}

impl<Brand> std::ops::Add<f64> for LinearExpression<Brand> {
    type Output = LinearExpression<Brand>;

    fn add(self, other: f64) -> Self::Output {
        LinearExpression {
            terms: self.terms,
            constant: self.constant + other,
        }
    }
}

impl<Brand> std::ops::Sub<LinearExpression<Brand>> for LinearExpression<Brand> {
    type Output = LinearExpression<Brand>;

    fn sub(self, other: LinearExpression<Brand>) -> Self::Output {
        let mut terms = self.terms;
        terms.extend(other.terms.into_iter().map(|term| LinearTerm {
            coefficient: -term.coefficient,
            variable: term.variable,
        }));
        LinearExpression {
            terms,
            constant: self.constant - other.constant,
        }
    }
}

impl<Brand> std::ops::Sub<VariableId<Brand>> for LinearExpression<Brand> {
    type Output = LinearExpression<Brand>;

    fn sub(self, other: VariableId<Brand>) -> Self::Output {
        self - LinearExpression::from_variable(other)
    }
}

impl<Brand> std::ops::Sub<f64> for LinearExpression<Brand> {
    type Output = LinearExpression<Brand>;

    fn sub(self, other: f64) -> Self::Output {
        LinearExpression {
            terms: self.terms,
            constant: self.constant - other,
        }
    }
}

impl<Brand> std::ops::Mul<f64> for LinearExpression<Brand> {
    type Output = LinearExpression<Brand>;

    fn mul(self, other: f64) -> Self::Output {
        LinearExpression {
            terms: self
                .terms
                .into_iter()
                .map(|term| LinearTerm {
                    coefficient: term.coefficient * other,
                    variable: term.variable,
                })
                .collect(),
            constant: self.constant * other,
        }
    }
}

impl<Brand> std::ops::Mul<LinearExpression<Brand>> for f64 {
    type Output = LinearExpression<Brand>;

    fn mul(self, other: LinearExpression<Brand>) -> Self::Output {
        other * self
    }
}

// ============================================================================
// Operators for VariableId
// ============================================================================

impl<Brand> std::ops::Add<LinearExpression<Brand>> for VariableId<Brand> {
    type Output = LinearExpression<Brand>;

    fn add(self, other: LinearExpression<Brand>) -> Self::Output {
        LinearExpression::from_variable(self) + other
    }
}

impl<Brand> std::ops::Add<VariableId<Brand>> for VariableId<Brand> {
    type Output = LinearExpression<Brand>;

    fn add(self, other: VariableId<Brand>) -> Self::Output {
        LinearExpression::from_variable(self) + LinearExpression::from_variable(other)
    }
}

impl<Brand> std::ops::Add<f64> for VariableId<Brand> {
    type Output = LinearExpression<Brand>;

    fn add(self, other: f64) -> Self::Output {
        LinearExpression::from_variable(self) + other
    }
}

impl<Brand> std::ops::Sub<VariableId<Brand>> for VariableId<Brand> {
    type Output = LinearExpression<Brand>;

    fn sub(self, other: VariableId<Brand>) -> Self::Output {
        LinearExpression::from_variable(self) - LinearExpression::from_variable(other)
    }
}

impl<Brand> std::ops::Sub<LinearExpression<Brand>> for VariableId<Brand> {
    type Output = LinearExpression<Brand>;

    fn sub(self, other: LinearExpression<Brand>) -> Self::Output {
        LinearExpression::from_variable(self) - other
    }
}

impl<Brand> std::ops::Sub<f64> for VariableId<Brand> {
    type Output = LinearExpression<Brand>;

    fn sub(self, other: f64) -> Self::Output {
        LinearExpression::from_variable(self) - other
    }
}

impl<Brand> std::ops::Mul<f64> for VariableId<Brand> {
    type Output = LinearExpression<Brand>;

    fn mul(self, other: f64) -> Self::Output {
        LinearExpression::from_variable(self) * other
    }
}

impl<Brand> std::ops::Mul<VariableId<Brand>> for f64 {
    type Output = LinearExpression<Brand>;

    fn mul(self, other: VariableId<Brand>) -> Self::Output {
        other * self
    }
}

// ============================================================================
// Reverse operators for f64
// ============================================================================

impl<Brand> std::ops::Add<VariableId<Brand>> for f64 {
    type Output = LinearExpression<Brand>;

    fn add(self, other: VariableId<Brand>) -> Self::Output {
        LinearExpression::from_variable(other) + self
    }
}

impl<Brand> std::ops::Sub<VariableId<Brand>> for f64 {
    type Output = LinearExpression<Brand>;

    fn sub(self, other: VariableId<Brand>) -> Self::Output {
        LinearExpression::from_variable(other) - self
    }
}

#[cfg(test)]
mod tests {
    use crate::lp_solver::{LPModelBuilder, VariableType};
    use crate::lp_model_builder;

    #[test]
    fn test_branded_type_safety() {
        // Create two separate builders with different brands
        let mut builder1 = lp_model_builder!();
        let mut builder2 = lp_model_builder!();

        let x = builder1.add_variable("x", VariableType::Continuous, 0.0, 10.0);
        let y = builder2.add_variable("y", VariableType::Continuous, 0.0, 10.0);

        // These should work fine
        let _expr1 = x + 2.0;
        let _expr2 = y * 3.0;

        // This would NOT compile (uncomment to verify):
        // let _mixed = x + y;  // ERROR: different brands
    }

    #[test]
    fn test_expression_operations() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable("x", VariableType::Continuous, 0.0, 10.0);
        let y = builder.add_variable("y", VariableType::Continuous, 0.0, 10.0);

        // Test that expressions work with the macro-created brand
        let expr = 2.0 * x + 3.0 * y + 5.0;
        assert_eq!(expr.constant, 5.0);
        assert_eq!(expr.terms.len(), 2);

        // Test various operations
        let expr2 = x + y;
        let expr3 = x - y;
        let expr4 = 2.0 * x;
        let expr5 = x * 2.0;

        assert_eq!(expr2.terms.len(), 2);
        assert_eq!(expr3.terms.len(), 2);
        assert_eq!(expr4.terms.len(), 1);
        assert_eq!(expr5.terms.len(), 1);
    }

    #[test]
    fn test_variable_id_debug() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable("x", VariableType::Continuous, 0.0, 10.0);
        
        // Test that VariableId can be debug printed
        let debug_str = format!("{:?}", x);
        assert!(debug_str.contains("VariableId"));
    }
}