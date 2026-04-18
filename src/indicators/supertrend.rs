//! SuperTrend overlay from ATR bands around HL2.

use crate::domain::Candle;

use super::atr_series;

#[derive(Clone, Debug, PartialEq)]
pub struct SuperTrendBar {
    pub line: f64,
    pub long: bool,
}

/// ATR period (e.g. 10) and band multiplier (e.g. 3.0).
pub fn supertrend_series(
    candles: &[Candle],
    atr_period: usize,
    mult: f64,
) -> Vec<Option<SuperTrendBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if n == 0 || atr_period == 0 || mult <= 0.0 {
        return out;
    }
    let atr = atr_series(candles, atr_period);
    let mut final_upper = vec![0.0_f64; n];
    let mut final_lower = vec![0.0_f64; n];
    let mut long;
    let mut st_line;
    for i in 0..n {
        let Some(a) = atr[i] else {
            continue;
        };
        let hl2 = (candles[i].high + candles[i].low) / 2.0;
        let upper = hl2 + mult * a;
        let lower = hl2 - mult * a;
        if i == 0 {
            final_upper[i] = upper;
            final_lower[i] = lower;
            long = candles[i].close >= lower;
            st_line = if long { lower } else { upper };
            out[i] = Some(SuperTrendBar {
                line: st_line,
                long,
            });
            continue;
        }
        let prev_close = candles[i - 1].close;
        final_upper[i] = if prev_close <= final_upper[i - 1] {
            upper.min(final_upper[i - 1])
        } else {
            upper
        };
        final_lower[i] = if prev_close >= final_lower[i - 1] {
            lower.max(final_lower[i - 1])
        } else {
            lower
        };
        let prev_long = out[i - 1].as_ref().map(|b| b.long).unwrap_or(true);
        long = if prev_long {
            !(candles[i].close < final_lower[i - 1])
        } else {
            candles[i].close > final_upper[i - 1]
        };
        st_line = if long { final_lower[i] } else { final_upper[i] };
        out[i] = Some(SuperTrendBar { line: st_line, long });
    }
    out
}
