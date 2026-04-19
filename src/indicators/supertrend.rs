//! SuperTrend overlay from ATR bands around HL2.

use crate::domain::Candle;

use super::atr_series;

#[derive(Clone, Debug, PartialEq)]
pub struct SuperTrendBar {
    pub line: f64,
    pub long: bool,
}

/// ATR period (e.g. 10) and band multiplier (e.g. 3.0).
pub fn supertrend_series(
    candles: &[Candle],
    atr_period: usize,
    mult: f64,
) -> Vec<Option<SuperTrendBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if n == 0 || atr_period == 0 || mult <= 0.0 {
        return out;
    }
    let atr = atr_series(candles, atr_period);
    let mut final_upper = vec![None; n];
    let mut final_lower = vec![None; n];
    let mut direction = vec![0_i8; n];

    for i in 0..n {
        let Some(a) = atr[i] else {
            continue;
        };
        let hl2 = (candles[i].high + candles[i].low) / 2.0;
        let basic_upper = hl2 + mult * a;
        let basic_lower = hl2 - mult * a;

        if i == 0 || atr[i - 1].is_none() {
            final_upper[i] = Some(basic_upper);
            final_lower[i] = Some(basic_lower);
            direction[i] = 1;
            out[i] = Some(SuperTrendBar {
                line: basic_upper,
                long: false,
            });
            continue;
        }

        let prev_upper = final_upper[i - 1].expect("previous upper band");
        let prev_lower = final_lower[i - 1].expect("previous lower band");
        let prev_close = candles[i - 1].close;

        final_upper[i] = Some(if basic_upper < prev_upper || prev_close > prev_upper {
            basic_upper
        } else {
            prev_upper
        });
        final_lower[i] = Some(if basic_lower > prev_lower || prev_close < prev_lower {
            basic_lower
        } else {
            prev_lower
        });

        let prev_line = out[i - 1].as_ref().expect("previous supertrend state").line;
        direction[i] = if (prev_line - prev_upper).abs() < f64::EPSILON {
            if candles[i].close > final_upper[i].expect("upper band") {
                -1
            } else {
                1
            }
        } else if candles[i].close < final_lower[i].expect("lower band") {
            1
        } else {
            -1
        };

        let line = if direction[i] < 0 {
            final_lower[i].expect("lower band")
        } else {
            final_upper[i].expect("upper band")
        };
        out[i] = Some(SuperTrendBar {
            line,
            long: direction[i] < 0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::supertrend_series;
    use crate::domain::Candle;

    fn candle_at(minute: i64, open: f64, high: f64, low: f64, close: f64) -> Candle {
        Candle {
            close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                + Duration::minutes(minute),
            open,
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
    fn supertrend_initializes_at_first_valid_atr_bar() {
        let candles = vec![
            candle_at(0, 10.0, 11.0, 9.0, 10.0),
            candle_at(15, 10.0, 12.0, 9.5, 11.0),
            candle_at(30, 11.0, 13.0, 10.0, 12.0),
            candle_at(45, 12.0, 14.0, 11.0, 13.0),
        ];

        let st = supertrend_series(&candles, 3, 2.0);
        assert_eq!(st[0], None);
        assert_eq!(st[1], None);
        assert!(st[2].is_some());
    }
}
