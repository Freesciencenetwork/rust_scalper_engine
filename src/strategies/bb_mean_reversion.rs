//! Long after a dip to the lower Bollinger band with RSI stabilising and reclaim of the mid-line.

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::default::gates::{has_history, no_runway_veto, unresolved_shock_veto};
use super::safety::common_veto_reasons;

pub const BB_MEAN_REVERSION_STRATEGY_ID: &str = "bb_mean_reversion";

#[derive(Clone, Debug)]
pub struct BbMeanReversionEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl BbMeanReversionEngine {
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

        if index < 1 {
            reasons.push("bb_mean_reversion_needs_prior_bar".to_string());
        } else {
            let vol = &frame.indicator_snapshot.volatility;
            let prev = &dataset.frames[index - 1];
            let prev_vol = &prev.indicator_snapshot.volatility;

            match (vol.bb_middle_20, vol.bb_lower_20) {
                (Some(mid), Some(lower)) => {
                    let close = frame.candle.close;
                    let rsi_now = frame.indicator_snapshot.momentum.rsi_14;
                    let rsi_prev = prev.indicator_snapshot.momentum.rsi_14;

                    let touched_lower = frame.candle.low <= lower * 1.000_000_1
                        || prev.candle.close <= prev_vol.bb_lower_20.unwrap_or(f64::MAX);

                    let reclaim = close > mid;
                    let rsi_ok = matches!(
                        (rsi_now, rsi_prev),
                        (Some(cur), Some(p)) if cur < 40.0 && cur > p && cur > 22.0
                    );

                    if !touched_lower {
                        reasons.push("no_lower_band_exposure".to_string());
                    }
                    if !reclaim {
                        reasons.push("close_not_above_bb_middle".to_string());
                    }
                    if !rsi_ok {
                        reasons.push("rsi_not_turning_from_dip".to_string());
                    }
                }
                _ => reasons.push("bollinger_bands_missing".to_string()),
            }
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
