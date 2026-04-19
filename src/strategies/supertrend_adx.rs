//! Long when SuperTrend is bullish and ADX/DMI agree there is directional strength.

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::default::gates::{has_history, no_runway_veto, unresolved_shock_veto};
use super::safety::common_veto_reasons;

pub const SUPERTREND_ADX_STRATEGY_ID: &str = "supertrend_adx";

const ADX_MIN: f64 = 20.0;

#[derive(Clone, Debug)]
pub struct SupertrendAdxEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl SupertrendAdxEngine {
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

        let vol = &frame.indicator_snapshot.volatility;
        let dir = &frame.indicator_snapshot.directional;

        let st_long = vol.supertrend_long == Some(true);
        if !st_long {
            reasons.push("supertrend_not_long".to_string());
        }

        let adx_ok = dir.adx_14.is_some_and(|adx| adx >= ADX_MIN);
        if !adx_ok {
            reasons.push("adx_below_threshold".to_string());
        }

        let dmi_ok = dir
            .di_plus
            .zip(dir.di_minus)
            .is_some_and(|(plus, minus)| plus > minus);
        if !dmi_ok {
            reasons.push("dmi_not_bullish".to_string());
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
