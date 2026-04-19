//! VIDYA (variable index dynamic average) using |CMO(9)| to scale EMA step.

use super::cmo_series;

/// `period` = VIDYA length; CMO uses fixed 9 bars for volatility scaling.
pub fn vidya_series(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut out = vec![0.0_f64; n];
    if n == 0 || period == 0 {
        return out;
    }
    let k = 2.0 / (period as f64 + 1.0);
    let cmo9 = cmo_series(closes, 9);
    out[0] = closes[0];
    for i in 1..n {
        let abs_c = cmo9[i].map(|c| c.abs() / 100.0).unwrap_or(0.0);
        let sc = (k * abs_c.max(0.0001)).min(1.0);
        out[i] = out[i - 1] + sc * (closes[i] - out[i - 1]);
    }
    out
}
