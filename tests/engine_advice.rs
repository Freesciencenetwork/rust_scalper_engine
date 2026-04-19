//! Human-facing “advice” over the same merge + dataset path as indicator evaluation, then the
//! pluggable [`binance_BTC::Strategy`] stack. **Intent only** (not a broker order).

use std::fs;
use std::path::Path;

use anyhow::Context as _;
use binance_BTC::domain::SystemMode;
use binance_BTC::strategy::decision::SignalDecision;
use binance_BTC::{DecisionMachine, MachineRequest, StrategyConfig, strategy_engine_for};
use serde::Deserialize;

#[derive(Debug, PartialEq)]
enum BtcAdvice {
    /// Strategy allows a long continuation setup (buy-stop trigger is defined).
    ConsiderBuyStopSetup { trigger_price: f64 },
    /// No new long; vetoes or risk state blocked the path.
    DoNotBuyBtc { reasons: Vec<String> },
}

fn advice_from_decision(decision: &SignalDecision) -> BtcAdvice {
    if decision.allowed {
        let trigger = decision
            .trigger_price
            .expect("allowed path should carry trigger_price in fixtures");
        BtcAdvice::ConsiderBuyStopSetup {
            trigger_price: trigger,
        }
    } else {
        BtcAdvice::DoNotBuyBtc {
            reasons: decision.reasons.clone(),
        }
    }
}

fn evaluate_last_bar(
    base_config: StrategyConfig,
    request: MachineRequest,
) -> anyhow::Result<SignalDecision> {
    let halt = request.runtime_state.halt_new_entries_flag != 0;
    let machine = DecisionMachine::new(base_config);
    let (config, dataset) = machine
        .prepare_dataset(request)
        .context("prepare_dataset")?;
    let last = dataset
        .frames
        .len()
        .checked_sub(1)
        .context("at least one closed bar")?;
    let mut engine = strategy_engine_for(&config)?;
    engine.set_system_mode(if halt {
        SystemMode::Halted
    } else {
        SystemMode::Active
    });
    engine.replay_failed_acceptance_window(0, last, &dataset);
    Ok(engine.decide(last, &dataset))
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

    let decision = evaluate_last_bar(config, request).expect("bullish continuation scenario");

    let advice = advice_from_decision(&decision);
    assert!(
        matches!(advice, BtcAdvice::ConsiderBuyStopSetup { .. }),
        "expected buy-stop setup advice, got {advice:?}; decision={decision:?}"
    );
    if let BtcAdvice::ConsiderBuyStopSetup { trigger_price } = advice {
        assert!(trigger_price > 0.0, "trigger should be positive");
    }
    assert!(decision.allowed, "fixture expects an allowed long path");
}

#[test]
fn engine_advises_do_not_buy_btc_when_macro_veto_fixture_blocks() {
    let dir = Path::new("tests/scenarios/macro_veto_block");
    let config: StrategyConfig = read_json(&dir.join("machine_config.json"));
    let request: MachineRequest = read_json(&dir.join("request.json"));

    let decision = evaluate_last_bar(config, request).expect("macro veto scenario");

    let advice = advice_from_decision(&decision);
    assert_eq!(
        advice,
        BtcAdvice::DoNotBuyBtc {
            reasons: vec!["macro_veto".to_string()]
        },
        "expected explicit do-not-buy advice with macro_veto; full decision={decision:?}"
    );
    assert!(
        !decision.allowed,
        "blocked path should not allow a new long entry"
    );
}
