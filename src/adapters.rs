//! Binance Spot adapters come from the **`binance_spot_candles`** crate on [crates.io](https://crates.io/crates/binance_spot_candles) (version pinned in `Cargo.toml`).

pub mod binance {
    pub use binance_spot_candles::adapters::binance::*;
}

pub use binance_spot_candles::adapters::traits;

pub use traits::{CandleSourceAdapter, SymbolMetadataAdapter};
