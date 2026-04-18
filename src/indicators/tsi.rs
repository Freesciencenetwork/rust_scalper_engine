//! True strength index: 100 × EMA(EMA(pc)) / EMA(EMA(|pc|)) with double smoothing.

use super::ema_series;

/// `r` / `s` commonly 25 / 13 (close-to-close momentum).
pub fn tsi_series(closes: &[f64], r: usize, s: usize) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if r == 0 || s == 0 || n < 2 {
        return out;
    }
    let mut pc = vec![0.0_f64; n];
    for i in 1..n {
        pc[i] = closes[i] - closes[i - 1];
    }
    let abs_pc: Vec<f64> = pc.iter().map(|x| x.abs()).collect();
    let ema1_pc = ema_series(&pc, r);
    let ema2_pc = ema_series(&ema1_pc, s);
    let ema1_a = ema_series(&abs_pc, r);
    let ema2_a = ema_series(&ema1_a, s);
    let warmup = r + s;
    for i in 0..n {
        if i + 1 < warmup {
            continue;
        }
        let den = ema2_a[i];
        if den.abs() < f64::EPSILON {
            out[i] = Some(0.0);
        } else {
            out[i] = Some(100.0 * (ema2_pc[i] / den));
        }
    }
    out
}
