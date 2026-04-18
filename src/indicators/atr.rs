use crate::domain::Candle;

pub fn atr_series(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; candles.len()];
    if period == 0 || candles.is_empty() {
        return result;
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

    if candles.len() < period {
        return result;
    }

    let first_atr = true_ranges[..period].iter().sum::<f64>() / period as f64;
    result[period - 1] = Some(first_atr);

    let mut prev_atr = first_atr;
    for index in period..candles.len() {
        let atr = (prev_atr * (period - 1) as f64 + true_ranges[index]) / period as f64;
        result[index] = Some(atr);
        prev_atr = atr;
    }

    result
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::atr_series;
    use crate::domain::Candle;

    fn candle_at(minute: i64, high: f64, low: f64, close: f64) -> Candle {
        Candle {
            close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                + Duration::minutes(minute),
            open: close,
            high,
            low,
            close,
            volume: 1.0,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        }
    }

    #[test]
    fn atr_uses_wilder_smoothing_after_initial_average() {
        let candles = vec![
            candle_at(0, 10.0, 8.0, 9.0),
            candle_at(15, 11.0, 8.0, 10.0),
            candle_at(30, 13.0, 9.0, 12.0),
            candle_at(45, 14.0, 10.0, 13.0),
        ];

        let atr = atr_series(&candles, 3);
        assert_eq!(atr[0], None);
        assert_eq!(atr[1], None);
        assert!((atr[2].expect("initial atr") - 3.0).abs() < 1e-9);
        assert!((atr[3].expect("smoothed atr") - (10.0 / 3.0)).abs() < 1e-9);
    }
}
