//! TRIX: % change of triple-smoothed EMA; signal = EMA of TRIX.

use super::ema_series;

pub fn trix_series(
    closes: &[f64],
    period: usize,
    signal_period: usize,
) -> (Vec<Option<f64>>, Vec<Option<f64>>) {
    let n = closes.len();
    let mut trix = vec![None; n];
    let mut trix_sig = vec![None; n];
    if period == 0 || signal_period == 0 || n < 2 {
        return (trix, trix_sig);
    }
    let e1 = ema_series(closes, period);
    let e2 = ema_series(&e1, period);
    let e3 = ema_series(&e2, period);
    let mut tr = vec![0.0_f64; n];
    for i in 1..n {
        let prev = e3[i - 1];
        tr[i] = if prev.abs() < f64::EPSILON {
            0.0
        } else {
            100.0 * (e3[i] - prev) / prev
        };
    }
    let sig = ema_series(&tr, signal_period);
    let warmup = 3 * period + signal_period;
    for i in 1..n {
        if i + 1 < warmup {
            continue;
        }
        trix[i] = Some(tr[i]);
        trix_sig[i] = Some(sig[i]);
    }
    (trix, trix_sig)
}
