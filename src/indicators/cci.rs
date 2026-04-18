//! Commodity channel index from typical price.

use crate::domain::Candle;

/// CCI with `period` lookback on typical price `(H+L+C)/3`.
pub fn cci_series(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 {
        return out;
    }
    let tp: Vec<f64> = candles
        .iter()
        .map(|c| (c.high + c.low + c.close) / 3.0)
        .collect();
    for i in 0..n {
        if i + 1 < period {
            continue;
        }
        let w = &tp[i + 1 - period..=i];
        let sma: f64 = w.iter().sum::<f64>() / period as f64;
        let mean_dev: f64 = w.iter().map(|x| (x - sma).abs()).sum::<f64>() / period as f64;
        if mean_dev < f64::EPSILON {
            out[i] = Some(0.0);
        } else {
            out[i] = Some((tp[i] - sma) / (0.015 * mean_dev));
        }
    }
    out
}
