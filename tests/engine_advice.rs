//! Human-facing “advice” over the public machine API: **consider a long (buy-stop) setup**
//! vs **do not buy / stand aside**. Same logic the HTTP `POST /v1/evaluate` handler runs.
//!
//! This is **intent only** (`arm_long_stop` = would arm a buy-stop per rules), not a broker order.

use std::fs;
use std::path::Path;

use binance_BTC::{DecisionMachine, MachineAction, MachineRequest, MachineResponse, StrategyConfig};
use serde::Deserialize;

#[derive(Debug, PartialEq)]
enum BtcAdvice {
    /// Strategy allows a long continuation setup; `arm_long_stop` is the machine action.
    ConsiderBuyStopSetup {
        trigger_price: f64,
    },
    /// No new long; vetoes or risk state blocked the path.
    DoNotBuyBtc {
        reasons: Vec<String>,
    },
}

fn advice_from_response(response: &MachineResponse) -> BtcAdvice {
    match (&response.action, response.decision.allowed) {
        (MachineAction::ArmLongStop, true) => {
            let trigger = response
                .decision
                .trigger_price
                .expect("allowed arm_long_stop should carry trigger_price in fixtures");
            BtcAdvice::ConsiderBuyStopSetup {
                trigger_price: trigger,
            }
        }
        (MachineAction::StandAside, _) | (MachineAction::ArmLongStop, false) => {
            BtcAdvice::DoNotBuyBtc {
                reasons: response.decision.reasons.clone(),
            }
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let body = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", path.display()));
    serde_json::from_str(&body)
        .unwrap_or_else(|error| panic!("failed to parse '{}': {error}", path.display()))
}

#[test]
fn engine_advises_buy_stop_setup_when_bullish_continuation_fixture_allows() {
    let dir = Path::new("tests/scenarios/bullish_continuation_entry");
    let config: StrategyConfig = read_json(&dir.join("machine_config.json"));
    let request: MachineRequest = read_json(&dir.join("request.json"));

    let machine = DecisionMachine::new(config);
    let response = machine
        .evaluate(request)
        .expect("bullish continuation scenario should evaluate");

    let advice = advice_from_response(&response);
    assert!(
        matches!(advice, BtcAdvice::ConsiderBuyStopSetup { .. }),
        "expected buy-stop setup advice, got {advice:?}; decision={:?}",
        response.decision
    );
    if let BtcAdvice::ConsiderBuyStopSetup { trigger_price } = advice {
        assert!(trigger_price > 0.0, "trigger should be positive");
    }
    assert!(
        matches!(response.action, MachineAction::ArmLongStop),
        "action should be arm_long_stop when advising a long setup"
    );
}

#[test]
fn engine_advises_do_not_buy_btc_when_macro_veto_fixture_blocks() {
    let dir = Path::new("tests/scenarios/macro_veto_block");
    let config: StrategyConfig = read_json(&dir.join("machine_config.json"));
    let request: MachineRequest = read_json(&dir.join("request.json"));

    let machine = DecisionMachine::new(config);
    let response = machine
        .evaluate(request)
        .expect("macro veto scenario should evaluate");

    let advice = advice_from_response(&response);
    assert_eq!(
        advice,
        BtcAdvice::DoNotBuyBtc {
            reasons: vec!["macro_veto".to_string()]
        },
        "expected explicit do-not-buy advice with macro_veto; full decision={:?}",
        response.decision
    );
    assert!(
        matches!(response.action, MachineAction::StandAside),
        "blocked path should stand aside, not arm a stop"
    );
}
