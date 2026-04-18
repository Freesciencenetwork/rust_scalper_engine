//! Bollinger %B from precomputed Bollinger bands.

use super::bollinger::BollingerBar;

pub fn bollinger_pct_b_series(bb: &[Option<BollingerBar>], closes: &[f64]) -> Vec<Option<f64>> {
    let n = closes.len().min(bb.len());
    let mut out = vec![None; n];
    for i in 0..n {
        let Some(b) = &bb[i] else { continue };
        let w = b.upper - b.lower;
        if w.abs() < f64::EPSILON {
            out[i] = Some(0.5);
        } else {
            out[i] = Some((closes[i] - b.lower) / w);
        }
    }
    out
}
