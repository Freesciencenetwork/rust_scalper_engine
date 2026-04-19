//! Chandelier exit: long stop = highest(high,n) − mult×ATR; short stop = lowest(low,n) + mult×ATR.

use crate::domain::Candle;

use super::atr_series;

#[derive(Clone, Debug, PartialEq)]
pub struct ChandelierBar {
    pub long_stop: f64,
    pub short_stop: f64,
}

pub fn chandelier_exit_series(
    candles: &[Candle],
    period: usize,
    atr_period: usize,
    atr_mult: f64,
) -> Vec<Option<ChandelierBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 || atr_period == 0 || n == 0 {
        return out;
    }
    let atr = atr_series(candles, atr_period);
    let start = period.max(atr_period).saturating_sub(1);
    for i in start..n {
        let w = &candles[i + 1 - period..=i];
        let hh = w.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        let ll = w.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let Some(a) = atr[i] else { continue };
        out[i] = Some(ChandelierBar {
            long_stop: hh - atr_mult * a,
            short_stop: ll + atr_mult * a,
        });
    }
    out
}
