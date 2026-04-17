use anyhow::Result;

use crate::domain::{Candle, SymbolFilters};

pub trait CandleSourceAdapter {
    fn parse_candles(payload: &str) -> Result<Vec<Candle>>;
}

pub trait SymbolMetadataAdapter {
    fn parse_symbol_filters(payload: &str, symbol: &str) -> Result<SymbolFilters>;
}
