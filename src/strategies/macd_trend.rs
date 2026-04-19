//! Long bias when 15m EMA trend is up and MACD is bullish (library / research strategy).

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::default::gates::{has_history, no_runway_veto, unresolved_shock_veto};
use super::safety::common_veto_reasons;

pub const MACD_TREND_STRATEGY_ID: &str = "macd_trend";

#[derive(Clone, Debug)]
pub struct MacdTrendEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl MacdTrendEngine {
    pub fn new(config: StrategyConfig) -> Self {
        Self {
            config,
            system_mode: SystemMode::Active,
            failed_acceptance: FailedAcceptanceState::default(),
        }
    }

    pub fn evaluate_signal(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        let frame = &dataset.frames_15m[index];
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
                atr: frame.atr_15m,
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
            .ema_fast_15m
            .zip(frame.ema_slow_15m)
            .is_some_and(|(fast, slow)| fast > slow);
        if !ema_up {
            reasons.push("ema_fast_not_above_slow".to_string());
        }

        let m = &frame.indicator_snapshot.momentum;
        let macd_bull = match (m.macd_line, m.macd_signal, m.macd_hist) {
            (Some(line), Some(sig), Some(hist)) => line > sig && hist > 0.0,
            (Some(line), Some(sig), None) => line > sig,
            _ => false,
        };
        if !macd_bull {
            reasons.push("macd_not_bullish".to_string());
        }

        SignalDecision {
            allowed: reasons.is_empty(),
            reasons,
            regime: Some(regime),
            trigger_price: Some(trigger_price),
            atr: frame.atr_15m,
        }
    }
}
