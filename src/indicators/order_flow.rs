//! Order flow indicators derived from per-bar buy / sell volume.
//!
//! All functions degrade gracefully when `buy_volume` / `sell_volume` are absent:
//! they return `None` rather than producing garbage values.

use crate::domain::Candle;

/// Rolling Order Flow Imbalance (OFI): `sum(delta) / sum(volume)` over `window` bars.
///
/// Result is clamped to `[-1.0, 1.0]`:
/// - `+1.0` → all aggression on the buy side over the window
/// - `-1.0` → all aggression on the sell side
/// - `None`  → no bar in the window has `buy_volume` / `sell_volume` data
///
/// Bars without delta data contribute `0.0` to the numerator but still
/// contribute their volume to the denominator, which conservatively dilutes
/// the signal rather than ignoring the bar entirely.
pub fn ofi_series(candles: &[Candle], window: usize) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if window == 0 || n == 0 {
        return out;
    }
    for i in 0..n {
        let start = i.saturating_sub(window - 1);
        let slice = &candles[start..=i];
        let mut sum_delta = 0.0_f64;
        let mut sum_vol = 0.0_f64;
        let mut has_delta = false;
        for c in slice {
            if let Some(d) = c.inferred_delta() {
                sum_delta += d;
                has_delta = true;
            }
            sum_vol += c.volume;
        }
        if has_delta && sum_vol > 0.0 {
            out[i] = Some((sum_delta / sum_vol).clamp(-1.0, 1.0));
        }
    }
    out
}

/// Rolling trade aggression ratio: average of `buy_volume / total_volume` over `window` bars.
///
/// Result is in `[0.0, 1.0]`:
/// - `> 0.5` → more aggressive buying than selling
/// - `< 0.5` → more aggressive selling
/// - `None`  → `buy_volume` not available in the window
pub fn aggression_ratio_series(candles: &[Candle], window: usize) -> Vec<Option<f64>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if window == 0 || n == 0 {
        return out;
    }
    for i in 0..n {
        let start = i.saturating_sub(window - 1);
        let slice = &candles[start..=i];
        let mut sum_ratio = 0.0_f64;
        let mut count = 0usize;
        for c in slice {
            if let Some(buy) = c.buy_volume {
                if c.volume > 0.0 {
                    sum_ratio += buy / c.volume;
                    count += 1;
                }
            }
        }
        if count > 0 {
            out[i] = Some(sum_ratio / count as f64);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn c(t: i64, vol: f64, buy: f64) -> Candle {
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        Candle {
            close_time: base + Duration::minutes(t),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: vol,
            buy_volume: Some(buy),
            sell_volume: Some(vol - buy),
            delta: None,
        }
    }

    #[test]
    fn ofi_all_buy_returns_plus_one() {
        let candles = vec![c(0, 10.0, 10.0), c(15, 10.0, 10.0), c(30, 10.0, 10.0)];
        let ofi = ofi_series(&candles, 3);
        let last = ofi.last().unwrap().unwrap();
        assert!((last - 1.0).abs() < 1e-9, "expected ~1.0, got {last}");
    }

    #[test]
    fn ofi_all_sell_returns_minus_one() {
        let candles = vec![c(0, 10.0, 0.0), c(15, 10.0, 0.0), c(30, 10.0, 0.0)];
        let ofi = ofi_series(&candles, 3);
        let last = ofi.last().unwrap().unwrap();
        assert!((last + 1.0).abs() < 1e-9, "expected ~-1.0, got {last}");
    }

    #[test]
    fn aggression_ratio_balanced_returns_half() {
        let candles = vec![c(0, 10.0, 5.0), c(15, 10.0, 5.0)];
        let r = aggression_ratio_series(&candles, 2);
        let last = r.last().unwrap().unwrap();
        assert!((last - 0.5).abs() < 1e-9, "expected 0.5, got {last}");
    }

    #[test]
    fn no_buy_volume_returns_none() {
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let candles = vec![Candle {
            close_time: base,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 10.0,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        }];
        let ofi = ofi_series(&candles, 1);
        assert!(ofi[0].is_none(), "expected None when no delta data");
        let ar = aggression_ratio_series(&candles, 1);
        assert!(ar[0].is_none(), "expected None when no buy_volume");
    }
}
