//! Chaikin oscillator: EMA(fast) of A/D line − EMA(slow) of A/D line.

use super::ema_series;

pub fn chaikin_oscillator_series(ad_line: &[f64], fast: usize, slow: usize) -> Vec<Option<f64>> {
    let n = ad_line.len();
    let mut out = vec![None; n];
    if fast == 0 || slow == 0 || n == 0 {
        return out;
    }
    let ef = ema_series(ad_line, fast);
    let es = ema_series(ad_line, slow);
    let warmup = slow.max(fast);
    for i in 0..n {
        if i + 1 < warmup {
            continue;
        }
        out[i] = Some(ef[i] - es[i]);
    }
    out
}

/// Convenience: builds cumulative A/D then oscillator (defaults 3 / 10).
pub fn chaikin_oscillator_from_candles(
    candles: &[crate::domain::Candle],
    fast: usize,
    slow: usize,
) -> Vec<Option<f64>> {
    use super::ad_line_series;
    let ad = ad_line_series(candles);
    chaikin_oscillator_series(&ad, fast, slow)
}
