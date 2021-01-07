use super::Symbol;

#[derive(PartialEq, Eq, Debug)]
pub enum EntryType {
    Port,
    DataReg,
    NullReg,
}

#[derive(PartialEq, Eq, Debug)]
pub struct Entry {
    pub entry_type: EntryType,
    pub name: Symbol,
    pub adjacency_list: Vec<Symbol>,
}

impl Entry {
    pub fn new(entry_type: EntryType, name: Symbol, adjacency_list: Vec<Symbol>) -> Entry {
        Entry {
            entry_type,
            name,
            adjacency_list,
        }
    }
}
