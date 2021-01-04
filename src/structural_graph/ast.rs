use immutable_string::ImmutableString;

#[derive(PartialEq, Eq, Debug)]
pub enum EntryType {
    Port,
    DataReg,
    NullReg,
}

#[derive(PartialEq, Eq, Debug)]
pub struct Entry {
    pub entry_type: EntryType,
    pub name: ImmutableString,
    pub adjacency_list: Vec<ImmutableString>,
}

impl Entry {
    pub fn new(
        entry_type: EntryType,
        name: ImmutableString,
        adjacency_list: Vec<ImmutableString>,
    ) -> Entry {
        Entry {
            entry_type,
            name,
            adjacency_list,
        }
    }
}
