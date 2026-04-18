//! Shared decision primitives (signals, sizing formulas, state) used by [`strategies`](crate::strategies).

pub mod decision;
pub mod formulas;
pub mod state;

pub use decision::SignalDecision;
