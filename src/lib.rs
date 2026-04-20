#![allow(non_snake_case)] // Cargo package name `binance_BTC`; changing it would break `use binance_BTC::…` for dependents.
#![allow(clippy::multiple_crate_versions)] // Transitive graph (e.g. `windows-sys`, `getrandom`); dedup belongs at workspace/lockfile policy, not here.

pub mod adapters;
pub mod catalog;
pub mod config;
pub mod context;
pub mod domain;
pub mod historical_data;
pub mod indicators;
pub mod machine;
pub mod market_data;
pub mod statistics;
pub mod strategies;
pub mod strategy;

pub use catalog::{
    CatalogIndicatorEntry, CatalogResponse, CatalogStrategyEntry, EngineSeriesSemantics,
    min_bars_required_for_path, path_note,
};
pub use config::{StrategyConfig, VwapAnchorMode};
pub use domain::{Candle, MacroEvent, SymbolFilters};
pub use historical_data::BundledBtcUsd1m;
pub use indicators::{VolumeProfileZones, volume_profile_zones};
pub use machine::{
    ConfigOverrides, DecisionMachine, EvaluateIndicatorError, EvaluateStrategyError,
    IndicatorEvaluateResponse, IndicatorReplayRequest, IndicatorReplayResponse,
    IndicatorReplayStep, IndicatorValueReport, MachineCapabilities, MachineRequest, RuntimeState,
    StrategyEvaluateResponse, StrategyReplayRequest, StrategyReplayResponse, StrategyReplayStep,
    SyntheticSeries,
};
pub use market_data::{
    CandlestickPatternSnapshot, DirectionalSnapshot, IchimokuSnapshot, IndicatorSnapshot,
    MomentumSnapshot, OrderFlowSnapshot, PivotClassicSnapshot, PivotFibSnapshot, PreparedCandle,
    PreparedDataset, TrendSnapshot, VolatilitySnapshot, VolumeSnapshot,
};
pub use strategies::default::StrategyEngine;
pub use strategies::supported_strategy_ids;
pub use strategies::{Strategy, strategy_engine_for};
