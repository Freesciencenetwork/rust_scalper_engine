//! Long on a 20-bar Donchian upper breakout with positive 20-period CMF (money flow confirmation).

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;
use crate::strategy::formulas::buy_stop_trigger_price;
use crate::strategy::state::FailedAcceptanceState;

use super::default::gates::{has_history, no_runway_veto, unresolved_shock_veto};
use super::safety::common_veto_reasons;

pub const DONCHIAN_BREAKOUT_STRATEGY_ID: &str = "donchian_breakout";

#[derive(Clone, Debug)]
pub struct DonchianBreakoutEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl DonchianBreakoutEngine {
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

        let close = frame.candle.close;
        let tol = (self.config.tick_size * 0.5).max(1e-6);
        let breakout = if index >= 20 {
            let prior_upper = dataset.frames[index - 20..index]
                .iter()
                .map(|f| f.candle.high)
                .fold(f64::NEG_INFINITY, f64::max);
            close > prior_upper - tol
        } else {
            false
        };
        if !breakout {
            reasons.push("close_not_at_donchian_upper".to_string());
        }

        let flow_ok = frame
            .indicator_snapshot
            .volume
            .cmf_20
            .is_some_and(|cmf| cmf > 0.0);
        if !flow_ok {
            reasons.push("cmf_not_positive".to_string());
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

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::DonchianBreakoutEngine;
    use crate::config::StrategyConfig;
    use crate::domain::Candle;
    use crate::market_data::{
        PreparedCandle, PreparedDataset,
        snapshot::{IndicatorSnapshot, VolumeSnapshot},
    };

    fn frame_at(minute: i64, high: f64, close: f64, cmf: f64) -> PreparedCandle {
        PreparedCandle {
            candle: Candle {
                close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                    + Duration::minutes(minute),
                open: close - 1.0,
                high,
                low: close - 2.0,
                close,
                volume: 1.0,
                buy_volume: None,
                sell_volume: None,
                delta: None,
            },
            ema_fast: None,
            ema_slow: None,
            ema_fast_higher: None,
            ema_slow_higher: None,
            vwma: None,
            atr: Some(10.0),
            atr_pct: None,
            atr_pct_baseline: Some(0.01),
            vol_ratio: Some(1.0),
            cvd_ema3: None,
            cvd_ema3_slope: None,
            vp_val: None,
            vp_poc: None,
            vp_vah: None,
            indicator_snapshot: IndicatorSnapshot {
                volume: VolumeSnapshot {
                    cmf_20: Some(cmf),
                    ..VolumeSnapshot::default()
                },
                ..IndicatorSnapshot::default()
            },
        }
    }

    #[test]
    fn donchian_breakout_checks_against_prior_channel() {
        let mut frames = Vec::new();
        for i in 0..20 {
            frames.push(frame_at(i * 15, 100.0 + i as f64, 99.0 + i as f64, 1.0));
        }
        frames.push(frame_at(20 * 15, 121.0, 121.0, 1.0));
        let dataset = PreparedDataset {
            frames,
            macro_events: Vec::new(),
        };
        let engine = DonchianBreakoutEngine::new(StrategyConfig {
            vol_baseline_lookback_bars: 1,
            vwma_lookback: 1,
            runway_lookback: 1,
            vp_enabled: false,
            ..StrategyConfig::default()
        });

        let decision = engine.evaluate_signal(20, &dataset);
        assert!(
            !decision
                .reasons
                .iter()
                .any(|reason| reason == "close_not_at_donchian_upper")
        );
    }
}
