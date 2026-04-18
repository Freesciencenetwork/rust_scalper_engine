//! Money flow index (typical price × volume; 0–100).

use crate::domain::Candle;

/// Standard MFI over `period` (commonly 14).
pub fn mfi_series(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 || n < period + 1 {
        return out;
    }
    let tp: Vec<f64> = candles
        .iter()
        .map(|c| (c.high + c.low + c.close) / 3.0)
        .collect();
    let mut pos_flow = vec![0.0_f64; n];
    let mut neg_flow = vec![0.0_f64; n];
    for i in 1..n {
        let raw_mf = tp[i] * candles[i].volume;
        if tp[i] > tp[i - 1] {
            pos_flow[i] = raw_mf;
        } else if tp[i] < tp[i - 1] {
            neg_flow[i] = raw_mf;
        }
    }
    for i in period..n {
        let p: f64 = pos_flow[i + 1 - period..=i].iter().sum();
        let neg: f64 = neg_flow[i + 1 - period..=i].iter().sum();
        let mfi = if neg < f64::EPSILON {
            100.0
        } else if p < f64::EPSILON {
            0.0
        } else {
            let mr = p / neg;
            100.0 - (100.0 / (1.0 + mr))
        };
        out[i] = Some(mfi);
    }
    out
}
