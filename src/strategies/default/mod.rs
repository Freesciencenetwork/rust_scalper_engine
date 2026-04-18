//! **Default** strategy: long-only BTC 15m bullish continuation pullback (original engine rules).

pub mod engine;
pub mod gates;

pub use engine::StrategyEngine;

/// `StrategyConfig.strategy_id` value for this implementation.
pub const DEFAULT_STRATEGY_ID: &str = "default";
