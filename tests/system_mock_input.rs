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
