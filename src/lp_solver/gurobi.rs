use std::collections::HashMap;

use ::gurobi::{attr, Env, Model, ModelSense, Status, Var, VarType, LinExpr, ConstrSense, Constr};

use crate::lp_solver::*;

/// Solve an LP model using Gurobi
pub fn solve_gurobi(builder: LPModelBuilder) -> Result<LPSolution> {
    let env = Env::new("")?;
    let mut model = Model::new("lp_model", &env)?;
    
    // Add variables
    let mut var_map = HashMap::new();
    for (var_id, (name, var_type, lower_bound, upper_bound)) in &builder.variables {
        let vtype = match var_type {
            VariableType::Continuous => VarType::Continuous,
            VariableType::Integer => VarType::Integer,
            VariableType::Binary => VarType::Binary,
        };
        
        let var = model.add_var(
            name,
            vtype,
            0.0, // objective coefficient
            *lower_bound,
            *upper_bound,
            &[], // coefficients for existing constraints
            &[], // constraint indices
        )?;
        
        var_map.insert(*var_id, var);
    }
    
    // Add constraints
    let mut constr_map = HashMap::new();
    for (constr_id, (name, expression, sense, rhs)) in builder.constraints.iter().enumerate() {
        let mut gurobi_expr = LinExpr::new();
        
        for term in &expression.terms {
            if let Some(var) = var_map.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(expression.constant);
        
        let sense = match sense {
            ConstraintSense::LessEqual => ConstrSense::Less,
            ConstraintSense::Equal => ConstrSense::Equal,
            ConstraintSense::GreaterEqual => ConstrSense::Greater,
            ConstraintSense::Greater => ConstrSense::Greater,
        };
        
        let constraint = model.add_constr(&name, gurobi_expr, sense, *rhs)?;
        constr_map.insert(ConstraintId(constr_id), constraint);
    }
    
    // Update the model before setting objective
    model.update()?;
    
    // Set objective
    if let Some((expression, sense)) = &builder.objective {
        let mut gurobi_expr = LinExpr::new();
        
        for term in &expression.terms {
            if let Some(var) = var_map.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(expression.constant);
        
        let sense = match sense {
            OptimizationSense::Minimize => ModelSense::Minimize,
            OptimizationSense::Maximize => ModelSense::Maximize,
        };
        
        model.set_objective(gurobi_expr, sense)?;
    }
    
    // Optimize
    model.optimize()?;
    
    // Get status
    let status = model.status()?;
    let optimization_status = match status {
        Status::Optimal | Status::SubOptimal => OptimizationStatus::Optimal,
        Status::Infeasible => OptimizationStatus::Infeasible,
        Status::Unbounded => OptimizationStatus::Unbounded,
        _ => OptimizationStatus::Other("Unknown status"),
    };
    
    // Extract variable values and objective value only if model is feasible
    let mut variable_values = HashMap::new();
    let objective_value = match optimization_status {
        OptimizationStatus::Optimal => {
            // Get variable values
            for (var_id, var) in &var_map {
                let value = var.get(&model, attr::X)?;
                variable_values.insert(*var_id, value);
            }
            
            // Get objective value
            model.get(attr::ObjVal)?
        },
        _ => {
            // For infeasible, unbounded, or other statuses, return default values
            0.0
        }
    };
    
    Ok(LPSolution {
        status: optimization_status,
        objective_value,
        variable_values,
    })
}

/// Legacy Gurobi solver implementation for backward compatibility
pub struct GurobiSolver;

impl LPSolver for GurobiSolver {
    fn new_model(&self, name: &str) -> Result<Box<dyn LPModel>> {
        Ok(Box::new(GurobiModel::new(name)?))
    }
}

/// Legacy Gurobi model implementation for backward compatibility
pub struct GurobiModel {
    env: Env,
    model: Model,
    variables: HashMap<VariableId, Var>,
    constraints: HashMap<ConstraintId, gurobi::Constr>,
    next_var_id: u32,
    next_constr_id: u32,
    solution: Option<LPSolution>,
}

impl GurobiModel {
    pub fn new(name: &str) -> Result<Self> {
        let env = Env::new("")?;
        let model = Model::new(name, &env)?;
        Ok(Self {
            env,
            model,
            variables: HashMap::new(),
            constraints: HashMap::new(),
            next_var_id: 0,
            next_constr_id: 0,
            solution: None,
        })
    }
}

impl LPModel for GurobiModel {
    fn add_variable(
        &mut self,
        name: &str,
        var_type: VariableType,
        lower_bound: f64,
        upper_bound: f64,
    ) -> Result<VariableId> {
        let var_id = VariableId(self.next_var_id as usize);
        self.next_var_id += 1;

        let vtype = match var_type {
            VariableType::Continuous => VarType::Continuous,
            VariableType::Integer => VarType::Integer,
            VariableType::Binary => VarType::Binary,
        };

        let var = self.model.add_var(
            name,
            vtype,
            0.0, // objective coefficient
            lower_bound,
            upper_bound,
            &[], // coefficients for existing constraints
            &[], // constraint indices
        )?;

        self.variables.insert(var_id, var);
        Ok(var_id)
    }

    fn add_constraint(
        &mut self,
        name: &str,
        expression: LinearExpression,
        sense: ConstraintSense,
        rhs: f64,
    ) -> Result<ConstraintId> {
        let constr_id = ConstraintId(self.next_constr_id as usize);
        self.next_constr_id += 1;

        let mut gurobi_expr = LinExpr::new();
        for term in &expression.terms {
            if let Some(var) = self.variables.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(expression.constant);

        let sense = match sense {
            ConstraintSense::LessEqual => ConstrSense::Less,
            ConstraintSense::Equal => ConstrSense::Equal,
            ConstraintSense::GreaterEqual => ConstrSense::Greater,
            ConstraintSense::Greater => ConstrSense::Greater,
        };

        let constraint = self.model.add_constr(name, gurobi_expr, sense, rhs)?;
        self.constraints.insert(constr_id, constraint);
        Ok(constr_id)
    }

    fn set_objective(&mut self, expression: LinearExpression, sense: OptimizationSense) -> Result<()> {
        let mut gurobi_expr = LinExpr::new();
        for term in &expression.terms {
            if let Some(var) = self.variables.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(anyhow::anyhow!("Variable {:?} not found in model", term.variable));
            }
        }
        gurobi_expr = gurobi_expr.add_constant(expression.constant);

        let sense = match sense {
            OptimizationSense::Minimize => ModelSense::Minimize,
            OptimizationSense::Maximize => ModelSense::Maximize,
        };

        self.model.set_objective(gurobi_expr, sense)?;
        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        self.model.update()?;
        Ok(())
    }

    fn optimize(&mut self) -> Result<()> {
        self.model.optimize()?;
        
        // Extract solution
        let status = self.model.status()?;
        let optimization_status = match status {
            Status::Optimal | Status::SubOptimal => OptimizationStatus::Optimal,
            Status::Infeasible => OptimizationStatus::Infeasible,
            Status::Unbounded => OptimizationStatus::Unbounded,
            _ => OptimizationStatus::Other("Unknown status"),
        };
        
        let mut variable_values = HashMap::new();
        let objective_value = match optimization_status {
            OptimizationStatus::Optimal => {
                // Get variable values
                for (var_id, var) in &self.variables {
                    let value = var.get(&self.model, attr::X)?;
                    variable_values.insert(*var_id, value);
                }
                
                // Get objective value
                self.model.get(attr::ObjVal)?
            },
            _ => {
                // For infeasible, unbounded, or other statuses, return default values
                0.0
            }
        };
        
        self.solution = Some(LPSolution {
            status: optimization_status,
            objective_value,
            variable_values,
        });
        
        Ok(())
    }

    fn status(&self) -> Result<OptimizationStatus> {
        if let Some(solution) = &self.solution {
            Ok(solution.status)
        } else {
            Ok(OptimizationStatus::Other("Not optimized"))
        }
    }

    fn get_variable_value(&self, variable: VariableId) -> Result<f64> {
        if let Some(solution) = &self.solution {
            solution.variable_values
                .get(&variable)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("Variable value not found in solution"))
        } else {
            Err(anyhow::anyhow!("Model not optimized or no solution available"))
        }
    }

    fn get_objective_value(&self) -> Result<f64> {
        if let Some(solution) = &self.solution {
            Ok(solution.objective_value)
        } else {
            Err(anyhow::anyhow!("Model not optimized or no solution available"))
        }
    }

    fn num_variables(&self) -> usize {
        self.variables.len()
    }

    fn num_constraints(&self) -> usize {
        self.constraints.len()
    }
}