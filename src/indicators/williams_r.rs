//! Williams %R over `period` highs/lows.

use crate::domain::Candle;

/// Range 0 (at period high) to −100 (at period low), classic definition.
pub fn williams_r_series(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 {
        return out;
    }
    for i in 0..n {
        if i + 1 < period {
            continue;
        }
        let w = &candles[i + 1 - period..=i];
        let hh = w.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        let ll = w.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let c = candles[i].close;
        let denom = hh - ll;
        if denom.abs() >= f64::EPSILON {
            out[i] = Some(-100.0 * (hh - c) / denom);
        }
    }
    out
}
