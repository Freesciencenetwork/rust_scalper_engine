//! Donchian channel: high/low over `period`; mid = average of band edges.

use crate::domain::Candle;

#[derive(Clone, Debug, PartialEq)]
pub struct DonchianBar {
    pub upper: f64,
    pub lower: f64,
    pub mid: f64,
}

pub fn donchian_series(candles: &[Candle], period: usize) -> Vec<Option<DonchianBar>> {
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
        let upper = w.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        let lower = w.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        out[i] = Some(DonchianBar {
            upper,
            lower,
            mid: 0.5 * (upper + lower),
        });
    }
    out
}
