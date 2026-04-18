//! Elder Ray bull / bear power vs EMA of close.

use crate::domain::Candle;

use super::ema_series;

pub fn elder_ray_series(candles: &[Candle], ema_period: usize) -> (Vec<f64>, Vec<f64>) {
    let n = candles.len();
    let mut bull = vec![0.0; n];
    let mut bear = vec![0.0; n];
    if n == 0 || ema_period == 0 {
        return (bull, bear);
    }
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let ema = ema_series(&closes, ema_period);
    for i in 0..n {
        bull[i] = candles[i].high - ema[i];
        bear[i] = candles[i].low - ema[i];
    }
    (bull, bear)
}
