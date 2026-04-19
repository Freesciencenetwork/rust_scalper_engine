//! Shared machine-level vetoes for alternate indicator strategies (mirrors
//! [`default::StrategyEngine::evaluate_common_blocks`](crate::strategies::default::StrategyEngine)).

use crate::config::StrategyConfig;
use crate::domain::{SystemMode, VolatilityRegime};
use crate::market_data::{PreparedCandle, PreparedDataset};

use super::default::gates::{active_regime, low_vol_floor_active, macro_event_veto, weekend_ban};

/// Reasons that block *any* entry plus the active volatility regime label.
pub(crate) fn common_veto_reasons(
    frame: &PreparedCandle,
    dataset: &PreparedDataset,
    config: &StrategyConfig,
    system_mode: SystemMode,
    trigger_price: Option<f64>,
    failed_acceptance_active: bool,
) -> (Vec<String>, VolatilityRegime) {
    let mut reasons = Vec::new();

    if system_mode == SystemMode::Halted {
        reasons.push("daily_halt".to_string());
    }
    if weekend_ban(frame.candle.close_time) {
        reasons.push("weekend_ban".to_string());
    }
    if macro_event_veto(frame.candle.close_time, &dataset.macro_events) {
        reasons.push("macro_veto".to_string());
    }
    if failed_acceptance_active {
        reasons.push("failed_acceptance".to_string());
    }

    let regime = active_regime(frame, config);
    if regime == VolatilityRegime::High {
        reasons.push("high_vol_regime".to_string());
    }
    if config.low_vol_enabled
        && let Some(entry_price) = trigger_price
        && low_vol_floor_active(frame, entry_price, config)
    {
        reasons.push("low_vol_floor".to_string());
    }

    (reasons, regime)
}
