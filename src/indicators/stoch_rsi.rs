//! Stochastic RSI: stochastic of RSI, then SMA smoothings (TradingView-style).

use super::rsi_series;

fn smooth_last(values: &[Option<f64>], i: usize, len: usize) -> Option<f64> {
    if i + 1 < len {
        return None;
    }
    let mut sum = 0.0;
    for v in &values[i + 1 - len..=i] {
        sum += v.as_ref()?;
    }
    Some(sum / len as f64)
}

/// `rsi_period` (e.g. 14), `stoch_period` on RSI extrema, then `k_smooth` / `d_smooth` SMAs.
pub fn stochastic_rsi_series(
    closes: &[f64],
    rsi_period: usize,
    stoch_period: usize,
    k_smooth: usize,
    d_smooth: usize,
) -> (Vec<Option<f64>>, Vec<Option<f64>>) {
    let n = closes.len();
    let mut k_out = vec![None; n];
    let mut d_out = vec![None; n];
    if rsi_period == 0 || stoch_period == 0 || k_smooth == 0 || d_smooth == 0 || n == 0 {
        return (k_out, d_out);
    }
    let rsi = rsi_series(closes, rsi_period);
    let mut raw = vec![None; n];
    for i in 0..n {
        if i + 1 < stoch_period {
            continue;
        }
        let mut mn = f64::INFINITY;
        let mut mx = f64::NEG_INFINITY;
        let mut ok = true;
        for r in &rsi[i + 1 - stoch_period..=i] {
            match r {
                Some(v) => {
                    mn = mn.min(*v);
                    mx = mx.max(*v);
                }
                None => ok = false,
            }
        }
        if !ok {
            continue;
        }
        let cur = rsi[i].expect("rsi");
        let denom = mx - mn;
        raw[i] = if denom.abs() < f64::EPSILON {
            None
        } else {
            Some((cur - mn) / denom * 100.0)
        };
    }
    for (i, kslot) in k_out.iter_mut().enumerate() {
        *kslot = smooth_last(&raw, i, k_smooth);
    }
    for (i, dslot) in d_out.iter_mut().enumerate() {
        *dslot = smooth_last(&k_out, i, d_smooth);
    }
    (k_out, d_out)
}
