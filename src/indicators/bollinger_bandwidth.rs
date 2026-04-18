//! Bollinger bandwidth: (upper − lower) / middle when middle ≠ 0.

use super::bollinger::BollingerBar;

pub fn bollinger_bandwidth_series(bb: &[Option<BollingerBar>]) -> Vec<Option<f64>> {
    let mut out = vec![None; bb.len()];
    for (i, b) in bb.iter().enumerate() {
        let Some(b) = b else { continue };
        if b.middle.abs() < f64::EPSILON {
            continue;
        }
        out[i] = Some((b.upper - b.lower) / b.middle);
    }
    out
}
