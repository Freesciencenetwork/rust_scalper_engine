//! Binance Spot public REST helpers: klines JSON, `exchangeInfo`, and CSV kline rows → [`Candle`] / [`SymbolFilters`].
//!
//! Publish this crate to crates.io; the workspace root `binance_BTC` engine is `publish = false`.

pub mod adapters;
pub mod domain;
