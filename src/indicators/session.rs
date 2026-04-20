//! Trading session classification and volume–trend confirmation.
//!
//! Sessions are based on UTC hour of each bar's `close_time`.
//! Sessions overlap intentionally (EU/US overlap 13:00–14:00 UTC).
//!
//! | Session | UTC hours |
//! |---------|-----------|
//! | Asia    | 00:00–08:00 |
//! | EU      | 07:00–14:00 |
//! | US      | 13:00–21:00 |

use chrono::Timelike;

use crate::domain::Candle;

/// Per-bar session membership flags.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionBar {
    pub in_asia_session: bool,
    pub in_eu_session: bool,
    pub in_us_session: bool,
}

/// Classify each bar into session flags based on UTC hour of `close_time`.
pub fn session_series(candles: &[Candle]) -> Vec<SessionBar> {
    candles
        .iter()
        .map(|c| {
            let h = c.close_time.hour();
            SessionBar {
                in_asia_session: h < 8,
                in_eu_session: h >= 7 && h < 14,
                in_us_session: h >= 13 && h < 21,
            }
        })
        .collect()
}

/// Rolling volume–trend confirmation: measures alignment between price direction
/// and volume delta direction over `window` bars.
///
/// Per-bar score:
/// - `+1.0` if price direction and delta direction agree (confirming move)
/// - `-1.0` if they disagree (divergence — potential reversal signal)
/// - ` 0.0` if either is flat
///
/// The series value is the rolling mean of per-bar scores.
/// Returns `None` when no bar in the window has delta data.
pub fn vol_trend_confirm_series(candles: &[Candle], window: usize) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if window == 0 || n < 2 {
        return out;
    }

    // Per-bar alignment score (None when delta unavailable).
    let per_bar: Vec<Option<f64>> = candles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                return None;
            }
            let delta = c.inferred_delta()?;
            let price_dir = c.close - candles[i - 1].close;
            let score = if price_dir > 0.0 && delta > 0.0 {
                1.0
            } else if price_dir < 0.0 && delta < 0.0 {
                1.0
            } else if (price_dir > 0.0 && delta < 0.0) || (price_dir < 0.0 && delta > 0.0) {
                -1.0
            } else {
                0.0
            };
            Some(score)
        })
        .collect();

    for i in 0..n {
        let start = i.saturating_sub(window - 1);
        let vals: Vec<f64> = per_bar[start..=i].iter().filter_map(|v| *v).collect();
        if !vals.is_empty() {
            out[i] = Some(vals.iter().sum::<f64>() / vals.len() as f64);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn c_at_hour(h: u32, close: f64, buy: f64, sell: f64) -> Candle {
        Candle {
            close_time: Utc.with_ymd_and_hms(2026, 1, 6, h, 0, 0).unwrap(),
            open: close - 1.0,
            high: close + 1.0,
            low: close - 2.0,
            close,
            volume: buy + sell,
            buy_volume: Some(buy),
            sell_volume: Some(sell),
            delta: None,
        }
    }

    #[test]
    fn session_classification_by_hour() {
        let candles = vec![
            c_at_hour(4, 100.0, 5.0, 5.0),  // Asia only
            c_at_hour(10, 100.0, 5.0, 5.0), // EU only
            c_at_hour(14, 100.0, 5.0, 5.0), // US only
            c_at_hour(13, 100.0, 5.0, 5.0), // EU+US overlap
        ];
        let s = session_series(&candles);
        assert!(s[0].in_asia_session && !s[0].in_eu_session && !s[0].in_us_session);
        assert!(!s[1].in_asia_session && s[1].in_eu_session && !s[1].in_us_session);
        assert!(!s[2].in_asia_session && !s[2].in_eu_session && s[2].in_us_session);
        assert!(!s[3].in_asia_session && s[3].in_eu_session && s[3].in_us_session);
    }

    #[test]
    fn vol_trend_confirm_confirmed_uptrend() {
        // Price up, delta positive each bar → confirm = +1
        let candles = vec![
            c_at_hour(12, 100.0, 6.0, 4.0),
            c_at_hour(12, 101.0, 7.0, 3.0),
            c_at_hour(12, 102.0, 8.0, 2.0),
        ];
        let vtc = vol_trend_confirm_series(&candles, 3);
        let last = vtc.last().unwrap().unwrap();
        assert!(last > 0.0, "confirmed uptrend should be positive, got {last}");
    }

    #[test]
    fn vol_trend_confirm_divergence_is_negative() {
        // Price up but delta negative → divergence
        let candles = vec![
            c_at_hour(12, 100.0, 5.0, 5.0),
            c_at_hour(12, 101.0, 2.0, 8.0), // price up, sell delta
            c_at_hour(12, 102.0, 2.0, 8.0),
        ];
        let vtc = vol_trend_confirm_series(&candles, 3);
        let last = vtc.last().unwrap().unwrap();
        assert!(last < 0.0, "divergence should be negative, got {last}");
    }
}
