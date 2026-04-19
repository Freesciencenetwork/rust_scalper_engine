//! Double exponential moving average.

use super::ema_series;

pub fn dema_series(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.is_empty() || period == 0 {
        return Vec::new();
    }
    let e1 = ema_series(closes, period);
    let e2 = ema_series(&e1, period);
    e1.iter().zip(e2.iter()).map(|(a, b)| 2.0 * a - b).collect()
}
