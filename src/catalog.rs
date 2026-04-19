//! Static catalog of strategies and flattened indicator paths for API discovery.

mod warmup;

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::config::StrategyConfig;
use crate::domain::Candle;
use crate::market_data::PreparedCandle;
use crate::strategies::supported_strategy_ids;

pub use warmup::{min_bars_required_for_path, path_note};

/// How the engine interprets each **row** in `candles` (see README).
#[derive(Clone, Debug, Serialize)]
pub struct EngineSeriesSemantics {
    pub uniform_bar_steps: bool,
    pub bar_interval_request_field: &'static str,
    pub higher_tf_factor: u32,
    pub detail: &'static str,
}

/// `GET /v1/catalog` payload.
#[derive(Clone, Debug, Serialize)]
pub struct CatalogResponse {
    pub engine_series_semantics: EngineSeriesSemantics,
    pub strategies: Vec<CatalogStrategyEntry>,
    /// Same paths as `indicators[].path` below (handy for older clients).
    pub indicator_paths: Vec<String>,
    pub indicators: Vec<CatalogIndicatorEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogStrategyEntry {
    pub id: String,
    pub description: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogIndicatorEntry {
    pub path: String,
    pub min_bars_required: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_note: Option<&'static str>,
}

fn strategy_description(id: &str) -> &'static str {
    match id {
        "default" => "Long-only 15m BTC continuation (project default).",
        "macd_trend" => "MACD line/signal trend-style engine.",
        "rsi_pullback" => "RSI pullback entries.",
        "supertrend_adx" => "SuperTrend + ADX filter.",
        "bb_mean_reversion" => "Bollinger mean-reversion style.",
        "stoch_crossover" => "Stochastic crossover engine.",
        "ichimoku_trend" => "Ichimoku cloud trend filter.",
        "ttm_squeeze_fire" => "TTM squeeze + momentum.",
        "donchian_breakout" => "Donchian channel breakout.",
        _ => "Strategy module.",
    }
}

/// Flatten a JSON object tree into dot-path keys (leaves only: numbers, bools, strings, null, arrays).
pub fn flatten_object_leaves(prefix: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                return;
            }
            for (key, child) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                match child {
                    Value::Object(sub) if !sub.is_empty() => {
                        flatten_object_leaves(&path, child, out);
                    }
                    Value::Object(_) => {}
                    _ => {
                        out.insert(path, child.clone());
                    }
                }
            }
        }
        _ => {
            out.insert(prefix.to_string(), value.clone());
        }
    }
}

/// `key` is included if any filter matches (empty `filters` → all keys).
pub fn key_matches_any_filter(key: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }
    filters.iter().any(|f| key_matches_filter(key, f))
}

fn key_matches_filter(key: &str, filter: &str) -> bool {
    if filter == "*" || filter.is_empty() {
        return true;
    }
    if key == filter {
        return true;
    }
    let prefix = format!("{filter}.");
    if key.starts_with(&prefix) {
        return true;
    }
    let suffix = format!(".{filter}");
    key.ends_with(&suffix)
}

pub fn filter_indicator_map(
    mut flat: BTreeMap<String, Value>,
    filters: &[String],
) -> (BTreeMap<String, Value>, Vec<String>) {
    if filters.is_empty() {
        return (flat, Vec::new());
    }
    let mut unmatched: Vec<String> = Vec::new();
    for f in filters {
        if !flat.keys().any(|k| key_matches_filter(k, f)) {
            unmatched.push(f.clone());
        }
    }
    flat.retain(|k, _| key_matches_any_filter(k, filters));
    (flat, unmatched)
}

fn sample_prepared_candle() -> PreparedCandle {
    let t = Utc.with_ymd_and_hms(2026, 1, 1, 0, 15, 0).unwrap();
    let candle = Candle {
        close_time: t,
        open: 1.0,
        high: 2.0,
        low: 0.5,
        close: 1.5,
        volume: 10.0,
        buy_volume: Some(6.0),
        sell_volume: Some(4.0),
        delta: None,
    };
    PreparedCandle {
        candle,
        ema_fast: None,
        ema_slow: None,
        ema_fast_higher: None,
        ema_slow_higher: None,
        vwma: None,
        atr: None,
        atr_pct: None,
        atr_pct_baseline: None,
        vol_ratio: None,
        cvd_ema3: None,
        cvd_ema3_slope: None,
        vp_val: None,
        vp_poc: None,
        vp_vah: None,
        indicator_snapshot: Default::default(),
    }
}

/// Build the catalog (strategy ids + all flattened paths for a schema [`PreparedCandle`]).
pub fn build_catalog_response() -> CatalogResponse {
    let default_cfg = StrategyConfig::default();
    let strategies = supported_strategy_ids()
        .iter()
        .map(|&id| CatalogStrategyEntry {
            id: id.to_string(),
            description: strategy_description(id),
        })
        .collect();
    let mut leaves = BTreeMap::new();
    let v = serde_json::to_value(sample_prepared_candle()).expect("serialize sample frame");
    flatten_object_leaves("", &v, &mut leaves);
    let indicator_paths: Vec<String> = leaves.keys().cloned().collect();
    let indicators: Vec<CatalogIndicatorEntry> = indicator_paths
        .iter()
        .map(|path| CatalogIndicatorEntry {
            path: path.clone(),
            min_bars_required: min_bars_required_for_path(path, &default_cfg),
            path_note: path_note(path),
        })
        .collect();
    CatalogResponse {
        engine_series_semantics: EngineSeriesSemantics {
            uniform_bar_steps: true,
            bar_interval_request_field: "bar_interval",
            higher_tf_factor: default_cfg.higher_tf_factor as u32,
            detail: "All indicator math is per row index. The bar_interval label is for your own logs only. The higher_tf_factor config controls how many base bars roll into one higher-TF bar for ema_fast_higher / ema_slow_higher.",
        },
        strategies,
        indicator_paths,
        indicators,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_includes_indicator_snapshot_leaf() {
        let mut m = BTreeMap::new();
        let v = serde_json::to_value(sample_prepared_candle()).unwrap();
        flatten_object_leaves("", &v, &mut m);
        assert!(m.contains_key(
            "indicator_snapshot.momentum.rsi_14"
        ) || m.keys().any(|k| k.ends_with(".rsi_14")));
    }
}
