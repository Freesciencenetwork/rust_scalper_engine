//! Exponential moving average of `volume`.

use crate::domain::Candle;

use super::ema_series;

pub fn volume_ema_series(candles: &[Candle], period: usize) -> Vec<f64> {
    let vols: Vec<f64> = candles.iter().map(|c| c.volume).collect();
    ema_series(&vols, period)
}
