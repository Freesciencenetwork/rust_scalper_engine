//! Long on slow-stochastic bullish cross from an oversold zone with 15m EMA trend up.

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::default::gates::{has_history, no_runway_veto, unresolved_shock_veto};
use super::safety::common_veto_reasons;

pub const STOCH_CROSSOVER_STRATEGY_ID: &str = "stoch_crossover";

const STOCH_OVERSOLD: f64 = 35.0;

#[derive(Clone, Debug)]
pub struct StochCrossoverEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl StochCrossoverEngine {
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

        if index < 1 {
            reasons.push("stoch_crossover_needs_prior_bar".to_string());
        } else {
            let m = &frame.indicator_snapshot.momentum;
            let prev_m = &dataset.frames_15m[index - 1].indicator_snapshot.momentum;
            let cross_up = match (m.stoch_k, m.stoch_d, prev_m.stoch_k, prev_m.stoch_d) {
                (Some(k), Some(d), Some(pk), Some(pd)) => {
                    k > d && pk <= pd && pk < STOCH_OVERSOLD && pd < STOCH_OVERSOLD
                }
                _ => false,
            };
            if !cross_up {
                reasons.push("stoch_bull_cross_missing".to_string());
            }
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
