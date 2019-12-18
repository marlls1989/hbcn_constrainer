use petgraph::prelude::*;
use std::collections::HashMap;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum NodeType {
    Port,
    LoopReg,
    DataReg,
    NullReg,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
            minDelay: minDelay,
        }
    }
}

pub struct StructuralGraph<'a> {
    node_type: HashMap<String, NodeType>,
    graph: DiGraphMap<&'a str, ()>,
}

impl StructuralGraph<'_> {
    pub fn new() -> Self {
        StructuralGraph {
            node_type: HashMap::new(),
            graph: DiGraphMap::new(),
        }
    }

    pub fn add_node(&mut self, nt: NodeType, name: &str) {
        let name = name.to_string();
        self.graph.add_node(&name);
        self.node_type.insert(name, nt);
    }

    pub fn add_edge(&mut self, src: &str, dst: &str) -> Result<(), String> {
        let src = match self.node_type.get_key_value(src) {
            Some((k, _)) => k,
            None => return Err(format!("src vertex {} not found", src)),
        };

        let dst = match self.node_type.get_key_value(dst) {
            Some((k, _)) => k,
            None => return Err(format!("dst vertex {} not found", dst)),
        };

        self.graph.add_edge(src, dst, ());

        return Ok(());
    }

    pub fn add_edges_to_src<'a, I>(&mut self, src: &str, dsts: I) -> Result<(), String>
    where
        I: Iterator<Item = &'a str>,
    {
        let src = match self.node_type.get_key_value(src) {
            Some((k, _)) => k,
            None => return Err(format!("src vertex {} not found", src)),
        };

        for dst in dsts {
            let dst = match self.node_type.get_key_value(dst) {
                Some((k, _)) => k,
                None => return Err(format!("dst vertex {} not found", dst)),
            };

            self.graph.add_edge(src, dst, ());
        }

        return Ok(());
    }
}
