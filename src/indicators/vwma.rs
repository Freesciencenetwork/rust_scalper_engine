use crate::domain::Candle;

pub fn vwma_series(candles: &[Candle], lookback: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; candles.len()];

    for index in 0..candles.len() {
        if index + 1 < lookback {
            continue;
        }
        let start = index + 1 - lookback;
        let window = &candles[start..=index];
        let total_volume: f64 = window.iter().map(|candle| candle.volume).sum();
        if total_volume <= 0.0 {
            continue;
        }
        let weighted_sum: f64 = window
            .iter()
            .map(|candle| candle.close * candle.volume)
            .sum();
        result[index] = Some(weighted_sum / total_volume);
    }

    result
}
