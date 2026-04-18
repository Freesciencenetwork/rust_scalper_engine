//! Weighted moving average (linear weights: oldest = 1, newest = period).

/// Values before `period - 1` are `None`. Denominator = `period * (period + 1) / 2`.
pub fn wma_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = values.len();
    let mut out = vec![None; n];
    if period == 0 {
        return out;
    }
    let denom = (period * (period + 1) / 2) as f64;
    for i in 0..n {
        if i + 1 < period {
            continue;
        }
        let mut num = 0.0;
        for (k, &v) in values[i + 1 - period..=i].iter().enumerate() {
            let w = (k + 1) as f64;
            num += w * v;
        }
        out[i] = Some(num / denom);
    }
    out
}
