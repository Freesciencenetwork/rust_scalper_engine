//! Arnaud Legoux moving average.

pub fn alma_series(closes: &[f64], window: usize, offset: f64, sigma: f64) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if window == 0 || sigma <= 0.0 || n < window {
        return out;
    }
    let m = offset * (window - 1) as f64;
    let s = window as f64 / sigma;
    let s2 = 2.0 * s * s;
    for i in window - 1..n {
        let w = &closes[i + 1 - window..=i];
        let mut num = 0.0_f64;
        let mut den = 0.0_f64;
        for (j, &price) in w.iter().enumerate() {
            let x = j as f64 - m;
            let coeff = (-(x * x) / s2).exp();
            num += price * coeff;
            den += coeff;
        }
        out[i] = Some(if den < f64::EPSILON {
            w[window - 1]
        } else {
            num / den
        });
    }
    out
}
