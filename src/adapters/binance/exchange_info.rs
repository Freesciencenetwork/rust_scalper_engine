use anyhow::{Context, Result, anyhow};

use crate::adapters::traits::SymbolMetadataAdapter;
use crate::domain::SymbolFilters;

use super::types::{BinanceExchangeInfo, BinanceFilter};

pub struct BinanceExchangeInfoAdapter;

impl SymbolMetadataAdapter for BinanceExchangeInfoAdapter {
    fn parse_symbol_filters(payload: &str, symbol: &str) -> Result<SymbolFilters> {
        let exchange_info: BinanceExchangeInfo =
            serde_json::from_str(payload).context("failed to parse Binance exchange info JSON")?;

        let symbol_info = exchange_info
            .symbols
            .into_iter()
            .find(|entry| entry.symbol == symbol)
            .ok_or_else(|| anyhow!("symbol '{symbol}' not found in Binance exchange info"))?;

        let mut tick_size = None;
        let mut lot_step = None;

        for filter in symbol_info.filters {
            match filter {
                BinanceFilter::PriceFilter { tick_size: value } => {
                    tick_size = Some(parse_decimal(&value)?);
                }
                BinanceFilter::LotSize { step_size: value } => {
                    lot_step = Some(parse_decimal(&value)?);
                }
                BinanceFilter::Other => {}
            }
        }

        Ok(SymbolFilters {
            tick_size: tick_size.ok_or_else(|| anyhow!("missing PRICE_FILTER tickSize"))?,
            lot_step: lot_step.ok_or_else(|| anyhow!("missing LOT_SIZE stepSize"))?,
        })
    }
}

fn parse_decimal(value: &str) -> Result<f64> {
    value
        .parse::<f64>()
        .with_context(|| format!("failed to parse decimal '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::BinanceExchangeInfoAdapter;
    use crate::adapters::traits::SymbolMetadataAdapter;

    #[test]
    fn parses_tick_and_step_size_from_exchange_info() {
        let filters = BinanceExchangeInfoAdapter::parse_symbol_filters(
            r#"{
              "symbols": [
                {
                  "symbol": "BTCUSDT",
                  "filters": [
                    {"filterType": "PRICE_FILTER", "tickSize": "0.10"},
                    {"filterType": "LOT_SIZE", "stepSize": "0.001"}
                  ]
                }
              ]
            }"#,
            "BTCUSDT",
        )
        .expect("filters");
        assert!((filters.tick_size - 0.1).abs() < 1e-9);
        assert!((filters.lot_step - 0.001).abs() < 1e-9);
    }
}
