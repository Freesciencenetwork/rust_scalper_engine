use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParameterOverlay {
    pub source_code: u16,
    pub report_timestamp_ms: i64,
    pub risk_fraction_multiplier: Option<f64>,
    pub high_vol_ratio_multiplier: Option<f64>,
    pub min_target_move_pct_multiplier: Option<f64>,
}
