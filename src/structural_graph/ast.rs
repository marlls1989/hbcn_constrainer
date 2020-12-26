use std::rc::Rc;

#[derive(PartialEq, Eq, Debug)]
pub enum EntryType {
    Port,
    DataReg,
    NullReg,
}

#[derive(PartialEq, Eq, Debug)]
pub struct Entry {
    pub entry_type: EntryType,
    pub name: Rc<str>,
    pub adjacency_list: Vec<Rc<str>>,
}

impl Entry {
    pub fn new(entry_type: EntryType, name: Rc<str>, adjacency_list: Vec<Rc<str>>) -> Entry {
        Entry {
            entry_type,
            name,
            adjacency_list,
        }
    }
}
