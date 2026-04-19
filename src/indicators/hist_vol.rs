//! Rolling sample stdev of log returns over `period` (not annualized).

pub fn hist_vol_log_returns_series(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if period < 2 || n < period + 1 {
        return out;
    }
    let mut lr = vec![0.0_f64; n];
    for i in 1..n {
        let a = closes[i - 1];
        lr[i] = if a.abs() < f64::EPSILON {
            0.0
        } else {
            (closes[i] / a).ln()
        };
    }
    for i in period..n {
        let w = &lr[i + 1 - period..=i];
        let mean: f64 = w.iter().sum::<f64>() / period as f64;
        let v: f64 = w
            .iter()
            .map(|x| {
                let d = x - mean;
                d * d
            })
            .sum::<f64>()
            / (period - 1).max(1) as f64;
        out[i] = Some(v.sqrt());
    }
    out
}
