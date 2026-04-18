//! Rate of change (%): `(close - close[n]) / close[n] * 100`.

pub fn roc_series(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if period == 0 {
        return out;
    }
    for i in period..n {
        let prev = closes[i - period];
        if prev.abs() < f64::EPSILON {
            out[i] = Some(0.0);
        } else {
            out[i] = Some((closes[i] - prev) / prev * 100.0);
        }
    }
    out
}
