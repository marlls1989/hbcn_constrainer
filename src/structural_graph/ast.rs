//! Abstract syntax tree (AST) representation for structural graph parsing.
//!
//! This module defines the intermediate representation used by the parser to build
//! structural graphs. The AST captures the parsed structure before it's converted
//! into the final graph representation.
//!
//! # AST Structure
//!
//! The AST consists of:
//!
//! - **[`EntryType`]**: The type of circuit component (Port, DataReg, etc.)
//! - **[`Entry`]**: A complete circuit component definition with its name and connections
//!
//! This AST is an implementation detail of the parser and is not typically used
//! by external code.

use super::Symbol;

/// Type of circuit component entry in the structural graph AST.
#[derive(PartialEq, Eq, Debug)]
pub enum EntryType {
    /// External port component.
    Port,
    /// Data register with internal structure.
    DataReg,
    /// Null register (simple register).
    NullReg,
    /// Control register (higher cost).
    ControlReg,
    /// Unsafe register with internal structure.
    UnsafeReg,
}

/// AST node representing a circuit component definition.
///
/// This captures the parsed structure of a single line in the structural graph format:
/// `<Type> <Name> [<adjacency_list>]`
///
/// # Fields
///
/// - `entry_type`: The component type
/// - `name`: Component identifier (symbol/interned string)
/// - `adjacency_list`: List of connections as `(target_name, delay)` tuples
#[derive(PartialEq, Debug)]
pub struct Entry {
    /// The type of circuit component.
    pub entry_type: EntryType,
    /// The name/identifier of the component.
    pub name: Symbol,
    /// List of connections: `(target_component_name, virtual_delay)`.
    pub adjacency_list: Vec<(Symbol, f64)>,
}

impl Entry {
    /// Create a new AST entry.
    ///
    /// This is used internally by the parser to construct AST nodes.
    pub fn new(entry_type: EntryType, name: Symbol, adjacency_list: Vec<(Symbol, f64)>) -> Entry {
        Entry {
            entry_type,
            name,
            adjacency_list,
        }
    }
}
