//! Binance Spot [`GET /api/v3/klines`](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/market-data-endpoints#klinecandlestick-data) client helpers.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{TimeZone, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::domain::Candle;

const DEFAULT_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Fetches closed klines from Binance Spot and returns normalized [`Candle`] rows (oldest first).
pub async fn fetch_klines(
    client: &Client,
    base_url: &str,
    symbol: &str,
    interval: &str,
    limit: u16,
    start_time_ms: Option<i64>,
    end_time_ms: Option<i64>,
) -> Result<Vec<Candle>> {
    let base = base_url.trim_end_matches('/');
    let mut url = format!(
        "{base}/api/v3/klines?symbol={symbol}&interval={interval}&limit={limit}"
    );
    if let Some(ms) = start_time_ms {
        url.push_str(&format!("&startTime={ms}"));
    }
    if let Some(ms) = end_time_ms {
        url.push_str(&format!("&endTime={ms}"));
    }

    let response = client
        .get(url)
        .send()
        .await
        .context("Binance klines HTTP request failed")?
        .error_for_status()
        .context("Binance klines returned an error status")?;

    let text = response
        .text()
        .await
        .context("failed to read Binance klines response body")?;

    parse_klines_json(&text)
}

/// Parses the JSON array returned by `/api/v3/klines` into [`Candle`] values.
pub fn parse_klines_json(payload: &str) -> Result<Vec<Candle>> {
    let rows: Vec<Value> =
        serde_json::from_str(payload).context("failed to parse Binance klines JSON")?;

    let mut candles = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let arr = row
            .as_array()
            .ok_or_else(|| anyhow!("klines row {index} is not a JSON array"))?;
        if arr.len() < 10 {
            bail!("klines row {index} has fewer than 10 fields");
        }

        let close_time_ms = value_as_i64(&arr[6]).with_context(|| format!("row {index} close_time"))?;
        let close_time = Utc
            .timestamp_millis_opt(close_time_ms)
            .single()
            .ok_or_else(|| anyhow!("row {index} has invalid close_time {close_time_ms}"))?;

        let volume = value_as_f64(&arr[5]).with_context(|| format!("row {index} volume"))?;
        let buy_volume = value_as_f64_opt(&arr[9]).with_context(|| format!("row {index} taker_buy_base"))?;
        let sell_volume = buy_volume.map(|buy| (volume - buy).max(0.0));

        candles.push(Candle {
            close_time,
            open: value_as_f64(&arr[1]).with_context(|| format!("row {index} open"))?,
            high: value_as_f64(&arr[2]).with_context(|| format!("row {index} high"))?,
            low: value_as_f64(&arr[3]).with_context(|| format!("row {index} low"))?,
            close: value_as_f64(&arr[4]).with_context(|| format!("row {index} close"))?,
            volume,
            buy_volume,
            sell_volume,
            delta: None,
        });
    }

    candles.sort_by_key(|c| c.close_time);
    Ok(candles)
}

/// Default [`Client`] for Binance calls (shared user-agent, reasonable defaults).
pub fn http_client() -> Result<Client> {
    Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .build()
        .context("failed to build HTTP client")
}

fn value_as_i64(value: &Value) -> Result<i64> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .ok_or_else(|| anyhow!("expected integer JSON number, got {n}")),
        Value::String(s) => s
            .parse::<i64>()
            .with_context(|| format!("expected integer string, got '{s}'")),
        _ => bail!("expected number or string for integer field, got {value}"),
    }
}

fn value_as_f64(value: &Value) -> Result<f64> {
    match value {
        Value::String(s) => s
            .parse::<f64>()
            .with_context(|| format!("expected decimal string, got '{s}'")),
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| anyhow!("expected JSON number with float representation, got {n}")),
        _ => bail!("expected number or string for float field, got {value}"),
    }
}

fn value_as_f64_opt(value: &Value) -> Result<Option<f64>> {
    match value {
        Value::Null => Ok(None),
        other => Ok(Some(value_as_f64(other)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_klines_json;

    #[test]
    fn parses_binance_kline_json_row() {
        // Field order per Binance Spot `GET /api/v3/klines` documentation.
        let payload = r#"[[1499040000000,"0.01634702","0.80000000","0.01575800","0.01577100","148976.11427815",1499644799999,"2434.19055334",308,"1756.87402397","28.46694399","17928899.62484339"]]"#;
        let candles = parse_klines_json(payload).expect("parse");
        assert_eq!(candles.len(), 1);
        assert!((candles[0].close - 0.01577100).abs() < 1e-8);
        assert!((candles[0].volume - 148976.11427815).abs() < 1e-6);
        assert!((candles[0].buy_volume.expect("buy") - 1756.87402397).abs() < 1e-6);
    }
}
