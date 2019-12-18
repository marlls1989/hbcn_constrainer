use std::collections::*;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum NodeLabel {
    Port,
    LoopReg,
    DataReg,
    NullReg,
}

#[derive(Clone,Debug)]
pub struct StructuralGraph<'a> where
{
    nodes: HashSet<String>,
    nodeid_map: HashMap<&'a str, usize>,
    adjacency_list: Vec<HashSet<&'a str>>,
    free_list: Vec<usize>,
}

impl Default for Graph<'_> {
    fn default() -> Self {
        StructuralGraph::new()
    }
}

impl StructuralGraph<'_> {
    pub fn new() -> Self {
        StructuralGraph{
            nodes: HashSet::new(),
            nodeid_map: HashMap::new(),
            adjacency_list: Vec::new(),
            free_list: Vec::new(),
        }
    }

    fn next_id(&mut self) -> usize {
        match self.free_list.pop() {
            Some(i) => i,
            None => self.adjacency_list.len(),
        }
    }


}
