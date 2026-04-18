use crate::domain::Candle;

pub fn vwma_series(candles: &[Candle], lookback: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; candles.len()];
    if lookback == 0 {
        return result;
    }

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

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::vwma_series;
    use crate::domain::Candle;

    fn candle_at(minute: i64, close: f64, volume: f64) -> Candle {
        Candle {
            close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                + Duration::minutes(minute),
            open: close,
            high: close,
            low: close,
            close,
            volume,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        }
    }

    #[test]
    fn returns_none_series_for_zero_lookback() {
        let candles = vec![candle_at(0, 100.0, 1.0)];
        assert_eq!(vwma_series(&candles, 0), vec![None]);
    }

    #[test]
    fn computes_volume_weighted_average() {
        let candles = vec![
            candle_at(0, 100.0, 1.0),
            candle_at(15, 110.0, 3.0),
        ];

        let vwma = vwma_series(&candles, 2);
        assert_eq!(vwma[0], None);
        assert!((vwma[1].expect("vwma") - 107.5).abs() < 1e-9);
    }
}
