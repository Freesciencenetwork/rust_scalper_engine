use crate::domain::Candle;

use super::ema::ema_series;

pub fn atr_series(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    if candles.is_empty() {
        return Vec::new();
    }

    let mut true_ranges = Vec::with_capacity(candles.len());
    true_ranges.push(candles[0].high - candles[0].low);

    for index in 1..candles.len() {
        let candle = &candles[index];
        let previous_close = candles[index - 1].close;
        let high_low = candle.high - candle.low;
        let high_close = (candle.high - previous_close).abs();
        let low_close = (candle.low - previous_close).abs();
        true_ranges.push(high_low.max(high_close).max(low_close));
    }

    let ema = ema_series(&true_ranges, period);
    ema.into_iter().map(Some).collect()
}
