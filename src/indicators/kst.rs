//! Know Sure Thing oscillator (Pring-style: sum of SMA-smoothed ROCs with 1,2,3,4 weights).

use super::roc_series;

fn sma_of_option_window(src: &[Option<f64>], i: usize, len: usize) -> Option<f64> {
    if i + 1 < len {
        return None;
    }
    let mut s = 0.0;
    for v in &src[i + 1 - len..=i] {
        s += *v.as_ref()?;
    }
    Some(s / len as f64)
}

fn roc_then_sma(closes: &[f64], roc_p: usize, sma_p: usize) -> Vec<Option<f64>> {
    let roc = roc_series(closes, roc_p);
    let n = closes.len();
    let mut out = vec![None; n];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = sma_of_option_window(&roc, i, sma_p);
    }
    out
}

/// KST line (no separate signal in snapshot).
pub fn kst_series(closes: &[f64]) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if n == 0 {
        return out;
    }
    let a = roc_then_sma(closes, 10, 10);
    let b = roc_then_sma(closes, 15, 10);
    let c = roc_then_sma(closes, 20, 10);
    let d = roc_then_sma(closes, 30, 10);
    for (i, slot) in out.iter_mut().enumerate() {
        if let (Some(x1), Some(x2), Some(x3), Some(x4)) = (a[i], b[i], c[i], d[i]) {
            *slot = Some(x1 + 2.0 * x2 + 3.0 * x3 + 4.0 * x4);
        }
    }
    out
}
