//! Ultimate oscillator (7 / 14 / 28), Larry Williams-style buying pressure ratios.

use crate::domain::Candle;

/// Returns 0–100. Uses sums of BP/TR over each lookback ending at bar `i`.
pub fn ultimate_oscillator_series(candles: &[Candle]) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if n < 2 {
        return out;
    }
    let (p7, p14, p28) = (7usize, 14usize, 28usize);
    let mut bp = vec![0.0_f64; n];
    let mut tr = vec![0.0_f64; n];
    for i in 1..n {
        let c = &candles[i];
        let p = &candles[i - 1];
        let tl = c.low.min(p.close);
        let th = c.high.max(p.close);
        bp[i] = c.close - tl;
        tr[i] = th - tl;
    }
    for i in (p28 - 1)..n {
        let s7: f64 = bp[i + 1 - p7..=i].iter().sum::<f64>()
            / tr[i + 1 - p7..=i].iter().sum::<f64>().max(f64::EPSILON);
        let s14: f64 = bp[i + 1 - p14..=i].iter().sum::<f64>()
            / tr[i + 1 - p14..=i].iter().sum::<f64>().max(f64::EPSILON);
        let s28: f64 = bp[i + 1 - p28..=i].iter().sum::<f64>()
            / tr[i + 1 - p28..=i].iter().sum::<f64>().max(f64::EPSILON);
        let uo = 100.0 * (4.0 * s7 + 2.0 * s14 + s28) / (4.0 + 2.0 + 1.0);
        out[i] = Some(uo.clamp(0.0, 100.0));
    }
    out
}
