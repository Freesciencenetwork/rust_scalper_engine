//! TTM Squeeze: Bollinger bands inside Keltner → compression; momentum = linreg of Carter/LazyBear source.

use crate::domain::Candle;

use super::{bollinger_series, keltner_series, sma_series};

#[derive(Clone, Debug, PartialEq)]
pub struct TtmSqueezeBar {
    pub squeezed: bool,
    pub momentum: Option<f64>,
}

fn linreg_endpoint_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = values.len();
    let mut out = vec![None; n];
    if period < 2 || n < period {
        return out;
    }
    let mean_x = (period - 1) as f64 / 2.0;
    let var_x: f64 = (0..period)
        .map(|i| {
            let d = i as f64 - mean_x;
            d * d
        })
        .sum();
    if var_x < f64::EPSILON {
        return out;
    }
    for i in period - 1..n {
        let window = &values[i + 1 - period..=i];
        let mean_y = window.iter().sum::<f64>() / period as f64;
        let mut cov = 0.0;
        for (j, &y) in window.iter().enumerate() {
            cov += (j as f64 - mean_x) * (y - mean_y);
        }
        let slope = cov / var_x;
        let intercept = mean_y - slope * mean_x;
        out[i] = Some(intercept + slope * (period - 1) as f64);
    }
    out
}

pub fn ttm_squeeze_series(candles: &[Candle]) -> Vec<Option<TtmSqueezeBar>> {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let bb = bollinger_series(&closes, 20, 2.0);
    let kc = keltner_series(candles, 20, 20, 1.5);
    let n = closes.len().min(bb.len()).min(kc.len());
    let mut out = vec![None; n];
    let sma_close = sma_series(&closes, 20);
    let mut source = vec![0.0_f64; n];

    for i in 0..n {
        if i + 1 < 20 {
            continue;
        }
        let window = &candles[i + 1 - 20..=i];
        let highest = window
            .iter()
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let lowest = window.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let basis = (highest + lowest) / 2.0;
        let avg = (basis + sma_close[i].expect("sma")) / 2.0;
        source[i] = closes[i] - avg;
    }

    let mom = linreg_endpoint_series(&source, 20);
    for i in 0..n {
        let (Some(b), Some(k)) = (&bb[i], &kc[i]) else {
            continue;
        };
        let squeezed = b.lower > k.lower && b.upper < k.upper;
        out[i] = Some(TtmSqueezeBar {
            squeezed,
            momentum: mom[i],
        });
    }
    out
}
