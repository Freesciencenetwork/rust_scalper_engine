use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RustyFishDailyReport {
    pub report_timestamp_ms: i64,
    pub trend_bias: f64,
    pub chop_bias: f64,
    pub vol_bias: f64,
    pub conviction: f64,
}
