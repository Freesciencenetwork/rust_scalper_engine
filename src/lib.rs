pub mod adapters;
pub mod config;
pub mod context;
pub mod domain;
pub mod indicators;
pub mod machine;
pub mod market_data;
pub mod strategies;
pub mod strategy;

pub use config::{StrategyConfig, VwapAnchorMode};
pub use domain::{Candle, MacroEvent, SymbolFilters};
pub use indicators::{VolumeProfileZones, volume_profile_zones};
pub use market_data::{
    DirectionalSnapshot, IchimokuSnapshot, IndicatorSnapshot, MomentumSnapshot, PivotClassicSnapshot,
    PivotFibSnapshot, PreparedCandle, PreparedDataset, TrendSnapshot, VolatilitySnapshot,
    VolumeSnapshot,
};
pub use machine::{
    ConfigOverrides, DecisionMachine, MachineAction, MachineCapabilities, MachineDiagnostics,
    MachineRequest, MachineResponse, RuntimeState,
};
pub use strategies::default::StrategyEngine;
pub use strategies::{strategy_engine_for, Strategy};
