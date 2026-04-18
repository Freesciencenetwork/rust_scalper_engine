//! Hull moving average: WMA(2·WMA(n/2) − WMA(n), √n) on closes.

use super::wma_series;

fn wma_last_window(window: &[f64]) -> f64 {
    let p = window.len();
    let denom = (p * (p + 1) / 2) as f64;
    let mut num = 0.0;
    for (k, &v) in window.iter().enumerate() {
        num += (k + 1) as f64 * v;
    }
    num / denom
}

/// `period` commonly 9 or 16. Returns `None` until enough history for all three WMAs.
pub fn hull_ma_series(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if period < 2 {
        return out;
    }
    let half = (period / 2).max(1);
    let sqrt_p = (period as f64).sqrt().round() as usize;
    let sqrt_p = sqrt_p.max(1);
    let w_half = wma_series(closes, half);
    let w_full = wma_series(closes, period);
    let mut raw = vec![0.0_f64; n];
    let mut raw_ok = vec![false; n];
    for i in 0..n {
        if let (Some(a), Some(b)) = (w_half[i], w_full[i]) {
            raw[i] = 2.0 * a - b;
            raw_ok[i] = true;
        }
    }
    for i in 0..n {
        if i + 1 < sqrt_p {
            continue;
        }
        let slice = &raw[i + 1 - sqrt_p..=i];
        let ok_slice = &raw_ok[i + 1 - sqrt_p..=i];
        if !ok_slice.iter().all(|&x| x) {
            continue;
        }
        out[i] = Some(wma_last_window(slice));
    }
    out
}
