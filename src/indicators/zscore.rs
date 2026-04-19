//! Rolling z-score of `values` vs SMA / sample stdev over `period`.

use super::sma_series;

pub fn zscore_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = values.len();
    let mut out = vec![None; n];
    if period < 2 || n < period {
        return out;
    }
    let sma = sma_series(values, period);
    for i in period - 1..n {
        let Some(mu) = sma[i] else { continue };
        let w = &values[i + 1 - period..=i];
        let v: f64 = w
            .iter()
            .map(|x| {
                let d = x - mu;
                d * d
            })
            .sum::<f64>()
            / (period - 1) as f64;
        let sd = v.sqrt();
        if sd < f64::EPSILON {
            out[i] = Some(0.0);
        } else {
            out[i] = Some((values[i] - mu) / sd);
        }
    }
    out
}
