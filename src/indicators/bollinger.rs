//! Bollinger bands: middle = SMA(n); width = k·sample stdev of closes.

use super::sma_series;

#[derive(Clone, Debug, PartialEq)]
pub struct BollingerBar {
    pub middle: f64,
    pub upper: f64,
    pub lower: f64,
}

fn stdev_sample(window: &[f64], mean: f64) -> f64 {
    if window.len() < 2 {
        return 0.0;
    }
    let v: f64 = window
        .iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / (window.len() - 1) as f64;
    v.sqrt()
}

/// `k` typically `2.0`. Uses sample standard deviation of closes in the window.
pub fn bollinger_series(
    closes: &[f64],
    period: usize,
    k: f64,
) -> Vec<Option<BollingerBar>> {
    let sma = sma_series(closes, period);
    let mut out = vec![None; closes.len()];
    for i in 0..closes.len() {
        let Some(mid) = sma[i] else { continue };
        if i + 1 < period {
            continue;
        }
        let w = &closes[i + 1 - period..=i];
        let sd = stdev_sample(w, mid);
        out[i] = Some(BollingerBar {
            middle: mid,
            upper: mid + k * sd,
            lower: mid - k * sd,
        });
    }
    out
}
