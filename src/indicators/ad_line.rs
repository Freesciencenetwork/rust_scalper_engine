//! Accumulation / distribution line (cumulative Marc Chaikin money flow).

use crate::domain::Candle;

/// Cumulative A/D from bar 0.
pub fn ad_line_series(candles: &[Candle]) -> Vec<f64> {
    let mut out = Vec::with_capacity(candles.len());
    let mut ad = 0.0;
    for c in candles {
        let h = c.high;
        let l = c.low;
        let range = h - l;
        let mfm = if range.abs() < f64::EPSILON * h.abs().max(1.0) {
            0.0
        } else {
            ((c.close - l) - (h - c.close)) / range
        };
        ad += mfm * c.volume;
        out.push(ad);
    }
    out
}
