use super::Symbol;

#[derive(PartialEq, Eq, Debug)]
pub enum EntryType {
    Port,
    DataReg,
    NullReg,
    ControlReg,
}

#[derive(PartialEq, Debug)]
pub struct Entry {
    pub entry_type: EntryType,
    pub name: Symbol,
    pub adjacency_list: Vec<(Symbol, f64)>,
}

impl Entry {
    pub fn new(entry_type: EntryType, name: Symbol, adjacency_list: Vec<(Symbol, f64)>) -> Entry {
        Entry {
            entry_type,
            name,
            adjacency_list,
        }
    }
}
