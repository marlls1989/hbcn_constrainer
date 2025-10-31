pub use super::super::DelayPair;
pub use crate::Symbol;


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Transition {
    Data(Symbol),
    Spacer(Symbol),
}

pub struct AdjencyEntry {
    pub source: Transition,
    pub delay: DelayPair,
    pub target: Transition,
    pub token: bool,
}

impl AdjencyEntry {
    pub fn new(source: Transition, delay: DelayPair, target: Transition, token: bool) -> Self {
        Self { source, delay, target, token }
    }
}

pub type AdjencyList = Vec<AdjencyEntry>;
