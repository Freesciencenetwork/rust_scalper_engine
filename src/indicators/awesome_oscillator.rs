//! Awesome oscillator: SMA(5) of median price − SMA(34) of median price.

use crate::domain::Candle;

use super::sma_series;

pub fn awesome_oscillator_series(candles: &[Candle]) -> Vec<Option<f64>> {
    let med: Vec<f64> = candles.iter().map(|c| (c.high + c.low) / 2.0).collect();
    let f5 = sma_series(&med, 5);
    let f34 = sma_series(&med, 34);
    med.iter()
        .enumerate()
        .map(|(i, _)| match (f5[i], f34[i]) {
            (Some(a), Some(b)) => Some(a - b),
            _ => None,
        })
        .collect()
}
