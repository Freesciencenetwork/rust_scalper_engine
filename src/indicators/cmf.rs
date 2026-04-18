//! Chaikin money flow: sum(MFM×V) / sum(V) over `period`.

use crate::domain::Candle;

pub fn cmf_series(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 {
        return out;
    }
    let mut mfmv = vec![0.0_f64; n];
    let mut v = vec![0.0_f64; n];
    for (i, c) in candles.iter().enumerate() {
        let h = c.high;
        let l = c.low;
        let range = h - l;
        let mfm = if range.abs() < f64::EPSILON * h.abs().max(1.0) {
            0.0
        } else {
            ((c.close - l) - (h - c.close)) / range
        };
        mfmv[i] = mfm * c.volume;
        v[i] = c.volume;
    }
    for i in (period - 1)..n {
        let num: f64 = mfmv[i + 1 - period..=i].iter().sum();
        let den: f64 = v[i + 1 - period..=i].iter().sum();
        out[i] = Some(if den < f64::EPSILON { 0.0 } else { num / den });
    }
    out
}
