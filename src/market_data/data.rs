use serde::{Deserialize, Serialize};

use crate::domain::{Candle, MacroEvent};

use super::snapshot::IndicatorSnapshot;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedCandle {
    pub candle: Candle,
    pub ema_fast_15m: Option<f64>,
    pub ema_slow_15m: Option<f64>,
    pub ema_fast_1h: Option<f64>,
    pub ema_slow_1h: Option<f64>,
    pub vwma_15m: Option<f64>,
    pub atr_15m: Option<f64>,
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
    pub frames_15m: Vec<PreparedCandle>,
    pub macro_events: Vec<MacroEvent>,
}
