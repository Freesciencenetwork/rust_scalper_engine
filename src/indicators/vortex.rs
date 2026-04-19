//! Vortex indicator VI+ and VI− over `period` (sum of directed movement / sum of true range).

use crate::domain::Candle;

#[derive(Clone, Debug, PartialEq)]
pub struct VortexBar {
    pub vi_plus: f64,
    pub vi_minus: f64,
}

pub fn vortex_series(candles: &[Candle], period: usize) -> Vec<Option<VortexBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 || n < 2 {
        return out;
    }
    let mut tr_s = vec![0.0_f64; n];
    let mut vm_p = vec![0.0_f64; n];
    let mut vm_m = vec![0.0_f64; n];
    for i in 1..n {
        let c = &candles[i];
        let p = &candles[i - 1];
        let tr = (c.high - c.low)
            .max((c.high - p.close).abs())
            .max((c.low - p.close).abs());
        tr_s[i] = tr;
        vm_p[i] = (c.high - p.low).abs();
        vm_m[i] = (c.low - p.high).abs();
    }
    for i in period..n {
        let sum_tr: f64 = tr_s[i + 1 - period..=i].iter().sum();
        let sum_p: f64 = vm_p[i + 1 - period..=i].iter().sum();
        let sum_m: f64 = vm_m[i + 1 - period..=i].iter().sum();
        if sum_tr < f64::EPSILON {
            continue;
        }
        out[i] = Some(VortexBar {
            vi_plus: sum_p / sum_tr,
            vi_minus: sum_m / sum_tr,
        });
    }
    out
}
