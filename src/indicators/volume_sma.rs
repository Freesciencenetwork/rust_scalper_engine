//! Simple moving average of `volume`.

use crate::domain::Candle;

use super::sma_series;

pub fn volume_sma_series(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    let vols: Vec<f64> = candles.iter().map(|c| c.volume).collect();
    sma_series(&vols, period)
}
