//! Binance Spot adapters live in workspace crate [`binance_spot_candles`](https://github.com/Freesciencenetwork/rust_scalper_engine/tree/main/crates/binance_spot_candles) (the publishable piece).

pub mod binance {
    pub use binance_spot_candles::adapters::binance::*;
}

pub use binance_spot_candles::adapters::traits;

pub use traits::{CandleSourceAdapter, SymbolMetadataAdapter};
