//! Triple exponential moving average.

use super::ema_series;

pub fn tema_series(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.is_empty() || period == 0 {
        return Vec::new();
    }
    let e1 = ema_series(closes, period);
    let e2 = ema_series(&e1, period);
    let e3 = ema_series(&e2, period);
    e1.iter()
        .zip(e2.iter())
        .zip(e3.iter())
        .map(|((a, b), c)| 3.0 * a - 3.0 * b + c)
        .collect()
}
