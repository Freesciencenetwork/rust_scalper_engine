use binance_BTC::{DecisionMachine, MachineRequest};
use serde_json::{Value, json};

fn build_mock_request_json(include_macro_event: bool) -> String {
    let base_close_time_ms = 1_744_873_700_000i64;

    let candles_15m: Vec<Value> = (0..970)
        .map(|index| {
            let trend = index as f64 * 0.8;
            let close = 80_000.0 + trend + if index == 969 { 3.0 } else { 0.0 };
            let open = close - if index == 969 { 2.0 } else { 1.0 };
            let high = close + 4.0;
            let low = if index == 969 {
                close - 6.0
            } else {
                open - 2.0
            };
            let volume = 100.0 + index as f64 * 0.2;
            let buy_volume = 60.0 + index as f64 * 0.1;
            let sell_volume = 40.0 + index as f64 * 0.05;

            json!({
                "close_time": base_close_time_ms + (index as i64 * 15 * 60 * 1000),
                "open": open,
                "high": high,
                "low": low,
                "close": close,
                "volume": volume,
                "buy_volume": buy_volume,
                "sell_volume": sell_volume,
                "delta": null
            })
        })
        .collect();

    let latest_close_time_ms = base_close_time_ms + (969i64 * 15 * 60 * 1000);
    let macro_events = if include_macro_event {
        vec![json!({
            "event_time": latest_close_time_ms + (10 * 60 * 1000),
            "class": 1
        })]
    } else {
        Vec::new()
    };

    json!({
        "candles_15m": candles_15m,
        "macro_events": macro_events,
        "runtime_state": {
            "realized_net_r_today": 0.0,
            "halt_new_entries_flag": 0
        },
        "account_equity": 100000.0,
        "symbol_filters": {
            "tick_size": 0.1,
            "lot_step": 0.001
        },
        "rustyfish_overlay": {
            "report_timestamp_ms": latest_close_time_ms - (6 * 60 * 60 * 1000),
            "trend_bias": 0.2,
            "chop_bias": 0.0,
            "vol_bias": 0.0,
            "conviction": 0.6
        }
    })
    .to_string()
}

#[test]
fn system_test_accepts_mock_json_and_returns_blocked_response() {
    let machine = DecisionMachine::default();
    let request_json = build_mock_request_json(true);

    let request: MachineRequest =
        serde_json::from_str(&request_json).expect("deserialize mock request");
    let response = machine.evaluate(request).expect("evaluate machine");
    let response_json = serde_json::to_value(&response).expect("serialize response");

    assert_eq!(response_json["action"], "stand_aside");
    assert_eq!(response_json["decision"]["allowed"], false);
    assert!(
        response_json["decision"]["reasons"]
            .as_array()
            .expect("reasons array")
            .iter()
            .any(|value| value == "macro_veto")
    );
    assert!(response_json["plan"].is_null());
    assert!(response_json["diagnostics"]["as_of"].as_i64().is_some());
    assert!(response_json["diagnostics"]["latest_frame"].is_object());
    assert!(response_json["diagnostics"]["effective_config"].is_object());
    assert!(response_json["diagnostics"]["overlay"].is_object());
}

#[test]
fn system_test_rejects_invalid_numeric_macro_event_code() {
    let request_json = build_mock_request_json(false);
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
