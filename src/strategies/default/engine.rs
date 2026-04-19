use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::domain::VolatilityRegime;
use crate::market_data::{PreparedCandle, PreparedDataset};
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::gates::{
    active_regime, context_favors_longs, flow_confirms, has_history, higher_timeframe_trend,
    low_vol_floor_active, lower_timeframe_trend, macro_event_veto, no_runway_veto,
    unresolved_shock_veto, update_failed_acceptance, valid_pullback, volume_profile_favors_longs,
    weekend_ban,
};

#[derive(Clone, Debug)]
pub struct StrategyEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl StrategyEngine {
    pub fn new(config: StrategyConfig) -> Self {
        Self {
            config,
            system_mode: SystemMode::Active,
            failed_acceptance: Default::default(),
        }
    }

    pub fn evaluate_signal(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        let frame = &dataset.frames[index];
        let trigger_price = buy_stop_trigger_price(frame.candle.high, self.config.tick_size);
        let mut decision = self.evaluate_common_blocks(index, dataset, Some(trigger_price));
        if !decision.allowed {
            return decision;
        }

        if !has_history(index, &self.config) {
            decision.allowed = false;
            decision.reasons.push("insufficient_history".to_string());
            return decision;
        }

        if !higher_timeframe_trend(frame) {
            decision.allowed = false;
            decision.reasons.push("1h_trend_not_bullish".to_string());
        }
        if !lower_timeframe_trend(index, &dataset.frames, self.config.trend_confirm_bars) {
            decision.allowed = false;
            decision.reasons.push("15m_trend_not_bullish".to_string());
        }
        if !context_favors_longs(frame) {
            decision.allowed = false;
            decision.reasons.push("below_vwma96".to_string());
        }
        if !volume_profile_favors_longs(frame, &self.config) {
            decision.allowed = false;
            decision
                .reasons
                .push("below_volume_profile_val".to_string());
        }
        if !valid_pullback(frame) {
            decision.allowed = false;
            decision.reasons.push("invalid_pullback".to_string());
        }
        if !flow_confirms(frame) {
            decision.allowed = false;
            decision.reasons.push("cvd_not_positive".to_string());
        }
        if no_runway_veto(index, dataset, trigger_price, &self.config) {
            decision.allowed = false;
            decision.reasons.push("no_runway".to_string());
        }
        if unresolved_shock_veto(index, dataset, &self.config) {
            decision.allowed = false;
            decision.reasons.push("unresolved_shock".to_string());
        }

        decision
    }

    pub(crate) fn evaluate_common_blocks(
        &self,
        index: usize,
        dataset: &PreparedDataset,
        trigger_price: Option<f64>,
    ) -> SignalDecision {
        let frame = &dataset.frames[index];
        let mut reasons = Vec::new();

        if self.system_mode == crate::domain::SystemMode::Halted {
            reasons.push("daily_halt".to_string());
        }
        if weekend_ban(frame.candle.close_time) {
            reasons.push("weekend_ban".to_string());
        }
        if macro_event_veto(frame.candle.close_time, &dataset.macro_events) {
            reasons.push("macro_veto".to_string());
        }
        if self.failed_acceptance.active {
            reasons.push("failed_acceptance".to_string());
        }

        let regime = self.active_regime(frame);
        if regime == VolatilityRegime::High {
            reasons.push("high_vol_regime".to_string());
        }
        if self.config.low_vol_enabled
            && let Some(entry_price) = trigger_price
            && low_vol_floor_active(frame, entry_price, &self.config)
        {
            reasons.push("low_vol_floor".to_string());
        }

        SignalDecision {
            allowed: reasons.is_empty(),
            reasons,
            regime: Some(regime),
            trigger_price,
            atr: frame.atr,
        }
    }

    pub(crate) fn active_regime(&self, frame: &PreparedCandle) -> VolatilityRegime {
        active_regime(frame, &self.config)
    }

    pub(crate) fn update_failed_acceptance(&mut self, index: usize, dataset: &PreparedDataset) {
        update_failed_acceptance(&mut self.failed_acceptance, index, dataset, &self.config);
    }
}
