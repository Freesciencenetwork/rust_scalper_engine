//! Shared decision primitives (signals, sizing formulas, state) used by [`strategies`](crate::strategies).

#![allow(clippy::pedantic, clippy::nursery)] // Sizing/volatility formulas; pedantic float/ naming churn for little gain.

pub mod decision;
pub mod formulas;
pub mod state;

pub use decision::SignalDecision;
