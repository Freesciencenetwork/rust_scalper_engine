#![allow(clippy::pedantic, clippy::nursery)] // Fixture JSON builders; pedantic float/cast churn in tests only.

use binance_BTC::MachineRequest;
use serde_json::{Value, json};

fn build_mock_request_json() -> String {
    let base_close_time_ms = 1_744_874_100_000i64;

    let candles: Vec<Value> = (0..10)
        .map(|index| {
            json!({
                "close_time": base_close_time_ms + (index as i64 * 15 * 60 * 1000),
                "open": 80_000.0 + index as f64,
                "high": 80_005.0 + index as f64,
                "low": 79_995.0 + index as f64,
                "close": 80_001.0 + index as f64,
                "volume": 100.0,
                "buy_volume": 60.0,
                "sell_volume": 40.0,
                "delta": null
            })
        })
        .collect();

    json!({
        "candles": candles,
        "macro_events": [],
        "runtime_state": {
            "realized_net_r_today": 0.0,
            "halt_new_entries_flag": 0
        },
        "account_equity": 100000.0,
        "symbol_filters": {
            "tick_size": 0.1,
            "lot_step": 0.001
        }
    })
    .to_string()
}

#[test]
fn system_test_request_deserializes_and_accepts_candles_alias() {
    // Verify candles_15m alias still parses as candles.
    let request_json = build_mock_request_json();
    let value: Value = serde_json::from_str(&request_json).expect("json");
    let parsed = serde_json::from_value::<MachineRequest>(value).expect("deserialize");
    assert_eq!(parsed.candles.len(), 10);
}

#[test]
fn system_test_bundled_btcusd_1m_deserializes() {
    let value = json!({
        "bar_interval": "1m",
        "bundled_btcusd_1m": { "from": "2012-01-01", "to": "2012-01-02" }
    });
    let parsed = serde_json::from_value::<MachineRequest>(value).expect("deserialize");
    assert!(parsed.candles.is_empty());
    let b = parsed.bundled_btcusd_1m.as_ref().expect("bundled");
    assert_eq!(b.from.as_deref(), Some("2012-01-01"));
    assert_eq!(b.to.as_deref(), Some("2012-01-02"));
    assert!(!b.all);
}

#[test]
fn system_test_synthetic_series_deserializes() {
    let value = json!({
        "bar_interval": "15m",
        "synthetic_series": { "bar_count": 50 }
    });
    let parsed = serde_json::from_value::<MachineRequest>(value).expect("deserialize");
    assert!(parsed.candles.is_empty());
    assert_eq!(
        parsed.synthetic_series.as_ref().expect("syn").bar_count,
        Some(50)
    );
}

#[test]
fn system_test_rejects_invalid_numeric_macro_event_code() {
    let request_json = build_mock_request_json();
    let mut value: Value = serde_json::from_str(&request_json).expect("request json value");
    value["macro_events"] = json!([
        {
            "event_time": 1745752500000i64,
            "class": 99
        }
    ]);

    let parsed = serde_json::from_value::<MachineRequest>(value);
    assert!(parsed.is_err());
}
