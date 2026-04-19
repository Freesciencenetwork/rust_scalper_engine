//! Long when price is above the Ichimoku cloud and Tenkan is above Kijun.

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::default::gates::{has_history, no_runway_veto, unresolved_shock_veto};
use super::safety::common_veto_reasons;

pub const ICHIMOKU_TREND_STRATEGY_ID: &str = "ichimoku_trend";

#[derive(Clone, Debug)]
pub struct IchimokuTrendEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl IchimokuTrendEngine {
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

        let ichi = &frame.indicator_snapshot.ichimoku;
        let close = frame.candle.close;

        let tenkan_kijun = ichi.tenkan_9.zip(ichi.kijun_26).is_some_and(|(t, k)| t > k);
        if !tenkan_kijun {
            reasons.push("tenkan_not_above_kijun".to_string());
        }

        let above_cloud = match (ichi.senkou_a_26, ichi.senkou_b_52) {
            (Some(a), Some(b)) => close > a.max(b),
            _ => false,
        };
        if !above_cloud {
            reasons.push("close_not_above_cloud".to_string());
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
