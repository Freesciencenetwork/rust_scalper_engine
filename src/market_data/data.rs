use serde::{Deserialize, Serialize};

use crate::domain::{Candle, MacroEvent};

use super::snapshot::IndicatorSnapshot;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedCandle {
    pub candle: Candle,
    pub ema_fast: Option<f64>,
    pub ema_slow: Option<f64>,
    pub ema_fast_higher: Option<f64>,
    pub ema_slow_higher: Option<f64>,
    pub vwma: Option<f64>,
    pub atr: Option<f64>,
    pub atr_pct: Option<f64>,
    pub atr_pct_baseline: Option<f64>,
    pub vol_ratio: Option<f64>,
    pub cvd_ema3: Option<f64>,
    pub cvd_ema3_slope: Option<f64>,
    pub vp_val: Option<f64>,
    pub vp_poc: Option<f64>,
    pub vp_vah: Option<f64>,
    pub indicator_snapshot: IndicatorSnapshot,
}

#[derive(Clone, Debug)]
pub struct PreparedDataset {
    pub frames: Vec<PreparedCandle>,
    pub macro_events: Vec<MacroEvent>,
}
