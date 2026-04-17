use crate::context::overlay::ParameterOverlay;

use super::report::RustyFishDailyReport;

pub fn map_report_to_overlay(report: &RustyFishDailyReport) -> ParameterOverlay {
    let trend = clamp(report.trend_bias);
    let chop = clamp(report.chop_bias);
    let vol = clamp(report.vol_bias);
    let conviction = clamp(report.conviction);

    let risk_fraction_multiplier = 1.0 - (0.30 * chop) - (0.20 * vol) + (0.10 * trend * conviction);
    let high_vol_ratio_multiplier = 1.0 - (0.15 * vol);
    let min_target_move_pct_multiplier = 1.0 + (0.20 * chop) + (0.20 * (1.0 - conviction));

    ParameterOverlay {
        source_code: 1,
        report_timestamp_ms: report.report_timestamp_ms,
        risk_fraction_multiplier: Some(risk_fraction_multiplier),
        high_vol_ratio_multiplier: Some(high_vol_ratio_multiplier),
        min_target_move_pct_multiplier: Some(min_target_move_pct_multiplier),
    }
}

fn clamp(value: f64) -> f64 {
    value.max(-1.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::{RustyFishDailyReport, map_report_to_overlay};

    #[test]
    fn maps_rustyfish_report_into_parameter_overlay() {
        let report = RustyFishDailyReport {
            report_timestamp_ms: 1_744_700_800_000,
            trend_bias: 0.5,
            chop_bias: 0.8,
            vol_bias: 0.3,
            conviction: 0.6,
        };

        let overlay = map_report_to_overlay(&report);
        assert_eq!(overlay.source_code, 1);
        assert!(overlay.risk_fraction_multiplier.expect("risk multiplier") < 1.0);
        assert!(
            overlay
                .min_target_move_pct_multiplier
                .expect("target multiplier")
                >= 1.0
        );
    }
}
