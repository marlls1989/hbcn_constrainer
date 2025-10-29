pub use super::super::DelayPair;
pub use crate::Symbol;


pub enum Transition {
    Data(Symbol),
    Spacer(Symbol),
}

pub struct AdjencyEntry {
    pub source: Transition,
    pub delay: DelayPair,
    pub target: Transition,
}

impl AdjencyEntry {
    pub fn new(source: Transition, delay: DelayPair, target: Transition) -> Self {
        Self { source, delay, target }
    }
}

pub type AdjencyList = Vec<AdjencyEntry>;
