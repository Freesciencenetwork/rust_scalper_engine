//! Long when 15m EMAs are bullish and RSI turns up from a mild oversold zone (classic pullback).

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::default::gates::{has_history, no_runway_veto, unresolved_shock_veto};
use super::safety::common_veto_reasons;

pub const RSI_PULLBACK_STRATEGY_ID: &str = "rsi_pullback";

#[derive(Clone, Debug)]
pub struct RsiPullbackEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl RsiPullbackEngine {
    pub fn new(config: StrategyConfig) -> Self {
        Self {
            config,
            system_mode: SystemMode::Active,
            failed_acceptance: FailedAcceptanceState::default(),
        }
    }

    pub fn evaluate_signal(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        let frame = &dataset.frames[index];
        let trigger_price = buy_stop_trigger_price(frame.candle.high, self.config.tick_size);

        let (mut reasons, regime) = common_veto_reasons(
            frame,
            dataset,
            &self.config,
            self.system_mode,
            Some(trigger_price),
            self.failed_acceptance.active,
        );
        if !reasons.is_empty() {
            return SignalDecision {
                allowed: false,
                reasons,
                regime: Some(regime),
                trigger_price: Some(trigger_price),
                atr: frame.atr,
            };
        }

        if !has_history(index, &self.config) {
            reasons.push("insufficient_history".to_string());
        }
        if no_runway_veto(index, dataset, trigger_price, &self.config) {
            reasons.push("no_runway".to_string());
        }
        if unresolved_shock_veto(index, dataset, &self.config) {
            reasons.push("unresolved_shock".to_string());
        }

        let ema_up = frame
            .ema_fast
            .zip(frame.ema_slow)
            .is_some_and(|(fast, slow)| fast > slow);
        if !ema_up {
            reasons.push("ema_fast_not_above_slow".to_string());
        }

        let rsi_now = frame.indicator_snapshot.momentum.rsi_14;
        let rsi_prev = index
            .checked_sub(1)
            .and_then(|i| dataset.frames[i].indicator_snapshot.momentum.rsi_14);

        let pullback_ok = match (rsi_now, rsi_prev) {
            (Some(cur), Some(prev)) => cur > prev && cur < 48.0 && cur > 28.0,
            _ => false,
        };
        if !pullback_ok {
            reasons.push("rsi_pullback_pattern_missing".to_string());
        }

        SignalDecision {
            allowed: reasons.is_empty(),
            reasons,
            regime: Some(regime),
            trigger_price: Some(trigger_price),
            atr: frame.atr,
        }
    }
}
