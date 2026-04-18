//! McGinley dynamic (McGinley, 1997).

pub fn mcginley_series(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    if n == 0 || period == 0 {
        return Vec::new();
    }
    let k = 0.6;
    let mut out = Vec::with_capacity(n);
    out.push(closes[0]);
    for i in 1..n {
        let prev = out[i - 1];
        let c = closes[i];
        let ratio = (c / prev).max(f64::EPSILON);
        let denom = k * period as f64 * ratio.powi(4);
        out.push(prev + (c - prev) / denom.max(f64::EPSILON));
    }
    out
}
