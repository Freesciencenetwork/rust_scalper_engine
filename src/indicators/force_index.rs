//! Elder force index: raw = Δclose × volume; optional EMA smoothing.

use crate::domain::Candle;

use super::ema_series;

pub fn force_index_series(candles: &[Candle], smooth: usize) -> Vec<f64> {
    let n = candles.len();
    let mut raw = vec![0.0_f64; n];
    for i in 1..n {
        raw[i] = (candles[i].close - candles[i - 1].close) * candles[i].volume;
    }
    if smooth == 0 {
        return raw;
    }
    ema_series(&raw, smooth)
}
