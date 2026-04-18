pub mod adapters;
pub mod config;
pub mod context;
pub mod domain;
pub mod indicators;
pub mod machine;
pub mod strategy;

pub use config::StrategyConfig;
pub use domain::{Candle, MacroEvent, SymbolFilters};
pub use machine::{
    ConfigOverrides, DecisionMachine, MachineAction, MachineCapabilities, MachineDiagnostics,
    MachineRequest, MachineResponse, RuntimeState,
};
