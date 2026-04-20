//! Liquidity void (thin zone) detection.
//!
//! A bar is flagged as a thin zone when:
//! 1. Its price range is wide relative to recent ATR   (`range ≥ atr_mult × ATR`)
//! 2. Volume is thin relative to the rolling average   (`volume < vol_threshold × avg_vol`)
//!
//! This combination identifies bars where price traversed a range quickly with
//! little participation — classic "void" behaviour that often acts as a
//! fast-refill target or fair-value gap in subsequent sessions.

use crate::domain::Candle;

/// Returns a `bool` per bar.
///
/// # Parameters
/// - `atr`: ATR series (same length as `candles`; `None` slots are skipped).
/// - `vol_window`: rolling window for the average volume denominator.
/// - `atr_mult`: range must be at least this multiple of ATR (e.g. `1.5`).
/// - `vol_threshold`: volume must be below this fraction of average volume (e.g. `0.7`).
pub fn thin_zone_series(
    candles: &[Candle],
    atr: &[Option<f64>],
    vol_window: usize,
    atr_mult: f64,
    vol_threshold: f64,
) -> Vec<bool> {
    let n = candles.len();
    debug_assert_eq!(atr.len(), n, "atr length must match candles");
    if vol_window == 0 || n == 0 {
        return vec![false; n];
    }

    // Rolling average volume — recomputed O(n·w) but w is typically small (20).
    let avg_vol: Vec<f64> = (0..n)
        .map(|i| {
            let start = i.saturating_sub(vol_window - 1);
            let slice = &candles[start..=i];
            slice.iter().map(|c| c.volume).sum::<f64>() / slice.len() as f64
        })
        .collect();

    let mut out = vec![false; n];
    for i in 0..n {
        let Some(atr_val) = atr[i] else { continue };
        let avg = avg_vol[i];
        if atr_val <= 0.0 || avg <= 0.0 {
            continue;
        }
        let range = candles[i].high - candles[i].low;
        let wide = range >= atr_mult * atr_val;
        let thin = candles[i].volume < vol_threshold * avg;
        out[i] = wide && thin;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn c(t: i64, high: f64, low: f64, vol: f64) -> Candle {
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        Candle {
            close_time: base + Duration::minutes(t),
            open: (high + low) / 2.0,
            high,
            low,
            close: (high + low) / 2.0,
            volume: vol,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        }
    }

    #[test]
    fn wide_range_low_vol_flags_thin_zone() {
        // 4 normal bars then 1 wide-range, low-vol bar.
        let candles = vec![
            c(0,  101.0, 99.0, 10.0),
            c(15, 101.0, 99.0, 10.0),
            c(30, 101.0, 99.0, 10.0),
            c(45, 101.0, 99.0, 10.0),
            c(60, 105.0, 95.0,  3.0), // range = 10, ATR ≈ 2 → 10 >= 1.5*2; vol 3 < 0.7*avg
        ];
        let atr: Vec<Option<f64>> = vec![None, None, None, Some(2.0), Some(2.0)];
        let result = thin_zone_series(&candles, &atr, 4, 1.5, 0.7);
        assert!(result[4], "bar 4 should be a thin zone");
    }

    #[test]
    fn normal_bar_not_flagged() {
        let candles: Vec<Candle> = (0..5).map(|i| c(i * 15, 101.0, 99.0, 10.0)).collect();
        let atr: Vec<Option<f64>> = vec![None, None, None, Some(2.0), Some(2.0)];
        let result = thin_zone_series(&candles, &atr, 4, 1.5, 0.7);
        // Range = 2, ATR = 2 → 2 >= 1.5*2 is false → not a void
        assert!(!result[4], "normal-range bar should not be thin zone");
    }
}
