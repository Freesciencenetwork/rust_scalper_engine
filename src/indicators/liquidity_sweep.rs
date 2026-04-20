//! Liquidity sweep (stop-run) detection.
//!
//! A sweep fires when a bar's extreme pierces through the prior `lookback` bars'
//! range extreme AND the volume delta confirms the direction:
//!
//! - **Sweep up** (`sweep_up = true`): `high > max(prior highs)` with non-negative delta —
//!   aggressive buyers clearing overhead stop liquidity.
//! - **Sweep down** (`sweep_down = true`): `low < min(prior lows)` with non-positive delta —
//!   aggressive sellers clearing below stop liquidity.
//!
//! When delta data is absent, the price breakout alone is sufficient
//! (conservative fallback so the indicator is usable on plain OHLCV data).

use crate::domain::Candle;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquiditySweepBar {
    pub sweep_up: bool,
    pub sweep_down: bool,
}

/// `lookback`: number of **prior** bars to measure the range extreme against.
/// Returns a vec of length `candles.len()`; the first `lookback` bars are always
/// `{ sweep_up: false, sweep_down: false }` (insufficient history).
pub fn liquidity_sweep_series(candles: &[Candle], lookback: usize) -> Vec<LiquiditySweepBar> {
    let n = candles.len();
    let mut out = vec![LiquiditySweepBar::default(); n];
    if lookback == 0 || n <= lookback {
        return out;
    }
    for i in lookback..n {
        let prior = &candles[i - lookback..i];
        let max_high = prior.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        let min_low = prior.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let cur = &candles[i];
        let delta = cur.inferred_delta();

        // Price breakout conditions
        let breaks_high = cur.high > max_high;
        let breaks_low = cur.low < min_low;

        // Delta confirmation (if available).  Absent delta → direction not disputed.
        let delta_confirms_up = delta.map_or(true, |d| d >= 0.0);
        let delta_confirms_down = delta.map_or(true, |d| d <= 0.0);

        out[i] = LiquiditySweepBar {
            sweep_up: breaks_high && delta_confirms_up,
            sweep_down: breaks_low && delta_confirms_down,
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn c(t: i64, high: f64, low: f64, buy: f64, sell: f64) -> Candle {
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        Candle {
            close_time: base + Duration::minutes(t),
            open: (high + low) / 2.0,
            high,
            low,
            close: (high + low) / 2.0,
            volume: buy + sell,
            buy_volume: Some(buy),
            sell_volume: Some(sell),
            delta: None,
        }
    }

    #[test]
    fn sweep_up_detected_when_high_breaks_prior_range() {
        // Prior 2 bars: highs of 101 and 102.  Next bar: high = 103 (new high, buy delta).
        let candles = vec![
            c(0, 101.0, 99.0, 6.0, 4.0),
            c(15, 102.0, 100.0, 6.0, 4.0),
            c(30, 103.0, 100.0, 8.0, 2.0), // sweep up
        ];
        let s = liquidity_sweep_series(&candles, 2);
        assert!(s[2].sweep_up, "expected sweep_up on bar 2");
        assert!(!s[2].sweep_down);
        assert!(!s[0].sweep_up);
        assert!(!s[1].sweep_up);
    }

    #[test]
    fn sweep_blocked_by_opposite_delta() {
        // High breaks but sell delta — bears absorbing, not a sweep up.
        let candles = vec![
            c(0, 101.0, 99.0, 5.0, 5.0),
            c(15, 102.0, 100.0, 5.0, 5.0),
            c(30, 103.0, 100.0, 2.0, 8.0), // high breaks, but sell delta
        ];
        let s = liquidity_sweep_series(&candles, 2);
        assert!(!s[2].sweep_up, "sell delta should block sweep_up");
    }

    #[test]
    fn insufficient_lookback_returns_false() {
        let candles = vec![c(0, 100.0, 99.0, 5.0, 5.0), c(15, 105.0, 98.0, 8.0, 2.0)];
        let s = liquidity_sweep_series(&candles, 3); // lookback > available
        assert!(s.iter().all(|b| !b.sweep_up && !b.sweep_down));
    }
}
