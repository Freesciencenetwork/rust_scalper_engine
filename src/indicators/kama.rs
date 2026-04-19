//! Kaufman adaptive moving average (efficiency ratio window `er_period`).

pub fn kama_series(closes: &[f64], er_period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut out = vec![0.0_f64; n];
    if n == 0 || er_period == 0 {
        return out;
    }
    out[0] = closes[0];
    if n == 1 {
        return out;
    }
    let fast_sc = 2.0 / (2.0 + 1.0);
    let slow_sc = 2.0 / (30.0 + 1.0);
    for i in 1..n {
        if i < er_period {
            out[i] = closes[i];
            continue;
        }
        let change = (closes[i] - closes[i - er_period]).abs();
        let mut vol = 0.0_f64;
        for j in i + 1 - er_period..=i {
            vol += (closes[j] - closes[j - 1]).abs();
        }
        let er = if vol < f64::EPSILON {
            0.0
        } else {
            change / vol
        };
        let sc = (er * (fast_sc - slow_sc) + slow_sc).powi(2);
        out[i] = out[i - 1] + sc * (closes[i] - out[i - 1]);
    }
    out
}
