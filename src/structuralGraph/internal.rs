use petgraph::prelude::*;

pub enum Node {
    Port(String, Vec<String>),
    LoopReg(String, Vec<String>),
    DataReg(String, Vec<String>),
    NullReg(String, Vec<String>),
}

#[derive(Copy,Hash,Eq,Ord)]
pub struct Edge {
    pub hasToken: bool,
    pub relax: bool,
    pub minDelay: bool,
}

impl Edge {
    pub fn new(hasToken: bool, relax: bool, minDelay: bool) -> Edge {
        Edge {
            hasToken: hasToken,
            relax: relax,
            minDelay: minDelay
        }
    }
}

pub type StructuralGraph = Graph<Vec<String>, Edge>;
