pub use super::super::DelayPair;
pub use crate::Symbol;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Transition {
    Data(Symbol),
    Spacer(Symbol),
}

pub struct AdjacencyEntry {
    pub source: Transition,
    pub delay: DelayPair,
    pub target: Transition,
    pub token: bool,
}

impl AdjacencyEntry {
    pub fn new(source: Transition, delay: DelayPair, target: Transition, token: bool) -> Self {
        Self {
            source,
            delay,
            target,
            token,
        }
    }
}

pub type AdjacencyList = Vec<AdjacencyEntry>;
