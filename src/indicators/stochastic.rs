//! Stochastic oscillator %K and %D (SMA of %K over `d_period`).

use crate::domain::Candle;

#[derive(Clone, Debug, PartialEq)]
pub struct StochasticBar {
    pub k: f64,
    pub d: f64,
}

/// `%K` over `k_period`; `%D` = simple mean of last `d_period` %K values.
pub fn stochastic_series(
    candles: &[Candle],
    k_period: usize,
    d_period: usize,
) -> Vec<Option<StochasticBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if k_period == 0 || d_period == 0 {
        return out;
    }
    let mut k_hist: Vec<Option<f64>> = vec![None; n];
    for i in 0..n {
        if i + 1 < k_period {
            continue;
        }
        let w = &candles[i + 1 - k_period..=i];
        let hh = w.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        let ll = w.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let c = candles[i].close;
        let denom = hh - ll;
        k_hist[i] = Some(if denom.abs() < f64::EPSILON {
            50.0
        } else {
            100.0 * (c - ll) / denom
        });
    }
    for i in 0..n {
        if i + 1 < k_period + d_period - 1 {
            continue;
        }
        let mut sum = 0.0;
        let mut cnt = 0usize;
        for j in i + 1 - d_period..=i {
            if let Some(k) = k_hist[j] {
                sum += k;
                cnt += 1;
            }
        }
        if cnt != d_period {
            continue;
        }
        let k = k_hist[i].expect("k at i");
        let d = sum / d_period as f64;
        out[i] = Some(StochasticBar { k, d });
    }
    out
}
