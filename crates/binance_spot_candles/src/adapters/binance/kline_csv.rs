use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use csv::ReaderBuilder;

use crate::adapters::traits::CandleSourceAdapter;
use crate::domain::Candle;

use super::types::BinanceKlineCsvRow;

pub struct BinanceKlineCsvAdapter;

impl CandleSourceAdapter for BinanceKlineCsvAdapter {
    fn parse_candles(payload: &str) -> Result<Vec<Candle>> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .from_reader(payload.as_bytes());

        let mut candles = Vec::new();
        for row in reader.deserialize::<BinanceKlineCsvRow>() {
            let row = row.context("failed to deserialize Binance kline row")?;
            let close_time = Utc
                .timestamp_millis_opt(row.close_time)
                .single()
                .ok_or_else(|| anyhow::anyhow!("invalid close_time {}", row.close_time))?;
            let volume = parse_decimal(&row.volume)?;
            let buy_volume = row
                .taker_buy_base_volume
                .as_deref()
                .map(parse_decimal)
                .transpose()?;
            let sell_volume = buy_volume.map(|buy| (volume - buy).max(0.0));

            candles.push(Candle {
                close_time,
                open: parse_decimal(&row.open)?,
                high: parse_decimal(&row.high)?,
                low: parse_decimal(&row.low)?,
                close: parse_decimal(&row.close)?,
                volume,
                buy_volume,
                sell_volume,
                delta: None,
            });
        }

        candles.sort_by_key(|candle| candle.close_time);
        Ok(candles)
    }
}

fn parse_decimal(value: &str) -> Result<f64> {
    value
        .parse::<f64>()
        .with_context(|| format!("failed to parse Binance decimal '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::BinanceKlineCsvAdapter;
    use crate::adapters::traits::CandleSourceAdapter;

    #[test]
    fn parses_binance_kline_csv_into_normalized_candles() {
        let candles = BinanceKlineCsvAdapter::parse_candles(
            "open_time,open,high,low,close,volume,close_time,quote_asset_volume,number_of_trades,taker_buy_base_volume,taker_buy_quote_volume,ignore\n\
             1710000000000,100,110,95,105,10,1710000899999,1000,42,6,600,\n",
        )
        .expect("load candles");
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].close, 105.0);
        assert_eq!(candles[0].buy_volume, Some(6.0));
        assert_eq!(candles[0].sell_volume, Some(4.0));
    }
}
