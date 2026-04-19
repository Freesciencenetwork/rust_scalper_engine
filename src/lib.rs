#![allow(non_snake_case)] // Cargo package name `binance_BTC`; changing it would break `use binance_BTC::…` for dependents.
#![allow(clippy::multiple_crate_versions)] // Transitive graph (e.g. `windows-sys`, `getrandom`); dedup belongs at workspace/lockfile policy, not here.

pub mod adapters;
pub mod config;
pub mod context;
pub mod domain;
pub mod indicators;
pub mod machine;
pub mod market_data;
pub mod statistics;
pub mod strategies;
pub mod strategy;

pub use config::{StrategyConfig, VwapAnchorMode};
pub use domain::{Candle, MacroEvent, SymbolFilters};
pub use indicators::{VolumeProfileZones, volume_profile_zones};
pub use machine::{
    ConfigOverrides, DecisionMachine, MachineAction, MachineCapabilities, MachineDiagnostics,
    MachineRequest, MachineResponse, RuntimeState,
};
pub use market_data::{
    CandlestickPatternSnapshot, DirectionalSnapshot, IchimokuSnapshot, IndicatorSnapshot,
    MomentumSnapshot, PivotClassicSnapshot, PivotFibSnapshot, PreparedCandle, PreparedDataset,
    TrendSnapshot, VolatilitySnapshot, VolumeSnapshot,
};
pub use strategies::default::StrategyEngine;
pub use strategies::{Strategy, strategy_engine_for};
