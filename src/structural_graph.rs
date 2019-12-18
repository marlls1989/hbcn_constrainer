mod internal;
pub use internal::*;
#[macro_use]
use lalrpop_util::*;

lalrpop_mod! {parser, "/structural_graph/parser.rs"}
pub use parser::*;
