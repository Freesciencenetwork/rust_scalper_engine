//! Mass Index: sum of single EMA ratio of range over `sum_period`.

use crate::domain::Candle;

use super::ema_series;

pub fn mass_index_series(
    candles: &[Candle],
    ema_period: usize,
    sum_period: usize,
) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if n == 0 || ema_period == 0 || sum_period == 0 {
        return out;
    }
    let hl: Vec<f64> = candles.iter().map(|c| c.high - c.low).collect();
    let e1 = ema_series(&hl, ema_period);
    let e2 = ema_series(&e1, ema_period);
    for (i, slot) in out.iter_mut().enumerate() {
        if i + 1 < sum_period {
            continue;
        }
        let mut sum = 0.0;
        for j in i + 1 - sum_period..=i {
            let b = e2[j];
            if b.abs() < f64::EPSILON {
                continue;
            }
            sum += e1[j] / b;
        }
        *slot = Some(sum);
    }
    out
}
