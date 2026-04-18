//! Aroon up / down (0–100): proximity of highest high / lowest low to the **latest** bar in the window.

use crate::domain::Candle;

#[derive(Clone, Debug, PartialEq)]
pub struct AroonBar {
    pub up: f64,
    pub down: f64,
}

/// `period` commonly 25. Uses last `period` bars ending at `i` (inclusive).
pub fn aroon_series(candles: &[Candle], period: usize) -> Vec<Option<AroonBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 {
        return out;
    }
    for i in (period - 1)..n {
        let w = &candles[i + 1 - period..=i];
        let mut idx_h = 0usize;
        let mut idx_l = 0usize;
        for (j, c) in w.iter().enumerate() {
            if c.high >= w[idx_h].high {
                idx_h = j;
            }
            if c.low <= w[idx_l].low {
                idx_l = j;
            }
        }
        // Bars ago from **current** bar (end of window): newest is index period-1.
        let bars_since_h = (period - 1) - idx_h;
        let bars_since_l = (period - 1) - idx_l;
        let up = 100.0 * (period - bars_since_h) as f64 / period as f64;
        let down = 100.0 * (period - bars_since_l) as f64 / period as f64;
        out[i] = Some(AroonBar { up, down });
    }
    out
}
