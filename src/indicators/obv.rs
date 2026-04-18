//! On-balance volume from close-to-close direction.

use crate::domain::Candle;

/// Cumulative OBV; starts from zero because the first bar has no previous close.
pub fn obv_series(candles: &[Candle]) -> Vec<f64> {
    let mut out = Vec::with_capacity(candles.len());
    let mut obv = 0.0;
    for (i, c) in candles.iter().enumerate() {
        if i > 0 {
            let prev = candles[i - 1].close;
            if c.close > prev {
                obv += c.volume;
            } else if c.close < prev {
                obv -= c.volume;
            }
        }
        out.push(obv);
    }
    out
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::obv_series;
    use crate::domain::Candle;

    fn candle_at(minute: i64, open: f64, close: f64, volume: f64) -> Candle {
        Candle {
            close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                + Duration::minutes(minute),
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            volume,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        }
    }

    #[test]
    fn obv_starts_at_zero_and_uses_close_to_close_direction() {
        let candles = vec![
            candle_at(0, 9.0, 10.0, 5.0),
            candle_at(15, 10.0, 12.0, 7.0),
            candle_at(30, 12.0, 11.0, 3.0),
            candle_at(45, 11.0, 11.0, 4.0),
        ];

        let obv = obv_series(&candles);
        assert_eq!(obv, vec![0.0, 7.0, 4.0, 4.0]);
    }
}
