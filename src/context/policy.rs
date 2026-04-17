use crate::config::StrategyConfig;

use super::overlay::ParameterOverlay;

pub fn apply_overlay_to_config(
    base: &StrategyConfig,
    overlay: &ParameterOverlay,
) -> StrategyConfig {
    let mut config = base.clone();

    if let Some(multiplier) = overlay.risk_fraction_multiplier {
        config.risk_fraction *= clamp(multiplier, 0.50, 1.00);
    }

    if let Some(multiplier) = overlay.high_vol_ratio_multiplier {
        config.high_vol_ratio *= clamp(multiplier, 0.85, 1.15);
    }

    if let Some(multiplier) = overlay.min_target_move_pct_multiplier {
        config.min_target_move_pct *= clamp(multiplier, 1.00, 1.40);
    }

    config
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

#[cfg(test)]
mod tests {
    use crate::config::StrategyConfig;

    use super::{ParameterOverlay, apply_overlay_to_config};

    #[test]
    fn overlay_is_bounded_by_policy_clamps() {
        let base = StrategyConfig::default();
        let overlay = ParameterOverlay {
            source_code: 1,
            report_timestamp_ms: 1_744_700_800_000,
            risk_fraction_multiplier: Some(2.0),
            high_vol_ratio_multiplier: Some(0.1),
            min_target_move_pct_multiplier: Some(5.0),
        };

        let adjusted = apply_overlay_to_config(&base, &overlay);
        assert!((adjusted.risk_fraction - base.risk_fraction).abs() < 1e-12);
        assert!((adjusted.high_vol_ratio - (base.high_vol_ratio * 0.85)).abs() < 1e-12);
        assert!((adjusted.min_target_move_pct - (base.min_target_move_pct * 1.40)).abs() < 1e-12);
    }
}
