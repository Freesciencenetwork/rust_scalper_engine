//! Negative / positive volume index (cumulative, starting at 1000).

pub fn nvi_pvi_series(closes: &[f64], volumes: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = closes.len().min(volumes.len());
    let mut nvi = vec![1000.0_f64; n];
    let mut pvi = vec![1000.0_f64; n];
    if n == 0 {
        return (nvi, pvi);
    }
    for i in 1..n {
        let prev_c = closes[i - 1];
        let pct = if prev_c.abs() < f64::EPSILON {
            0.0
        } else {
            (closes[i] - prev_c) / prev_c
        };
        nvi[i] = nvi[i - 1];
        pvi[i] = pvi[i - 1];
        if volumes[i] < volumes[i - 1] {
            nvi[i] = nvi[i - 1] * (1.0 + pct);
        }
        if volumes[i] > volumes[i - 1] {
            pvi[i] = pvi[i - 1] * (1.0 + pct);
        }
    }
    (nvi, pvi)
}
