//! Session / rolling VWAP with optional volume-weighted σ bands.
//!
//! **Timeframe notes:**
//! - `UtcDay` anchor resets at each UTC midnight. Correct for sub-daily bars (1m–4h).
//!   For daily bars it resets every bar (= typical price per bar, no smoothing).
//!   For weekly+ bars the same applies — use `RollingBars` instead.
//! - `RollingBars` is fully timeframe-agnostic (count-based window).
//! - `Disabled` suppresses the field entirely.

use chrono::NaiveDate;

use crate::config::VwapAnchorMode;
use crate::domain::Candle;

#[derive(Clone, Debug, PartialEq)]
pub struct VwapBar {
    pub vwap: f64,
    pub upper_1sd: f64,
    pub lower_1sd: f64,
    pub upper_2sd: f64,
    pub lower_2sd: f64,
}

fn typical(c: &Candle) -> f64 {
    (c.high + c.low + c.close) / 3.0
}

fn bands_from_sums(sum_w: f64, sum_wp: f64, sum_wp2: f64) -> Option<VwapBar> {
    if sum_w < f64::EPSILON {
        return None;
    }
    let vwap = sum_wp / sum_w;
    let ex2 = sum_wp2 / sum_w;
    let var = (ex2 - vwap * vwap).max(0.0);
    let sd = var.sqrt();
    Some(VwapBar {
        vwap,
        upper_1sd: vwap + sd,
        lower_1sd: vwap - sd,
        upper_2sd: vwap + 2.0 * sd,
        lower_2sd: vwap - 2.0 * sd,
    })
}

/// `rolling_bars` used only when `mode == RollingBars`.
pub fn vwap_bands_series(
    candles: &[Candle],
    mode: VwapAnchorMode,
    rolling_bars: Option<usize>,
    include_current_bar: bool,
) -> Vec<Option<VwapBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if n == 0 || matches!(mode, VwapAnchorMode::Disabled) {
        return out;
    }
    match mode {
        VwapAnchorMode::UtcDay => utc_day_vwap(candles, include_current_bar, &mut out),
        VwapAnchorMode::RollingBars => {
            let rb = rolling_bars.unwrap_or(96).max(1);
            rolling_vwap(candles, rb, include_current_bar, &mut out);
        }
        VwapAnchorMode::Disabled => {}
    }
    out
}

fn utc_day_vwap(candles: &[Candle], include_current: bool, out: &mut [Option<VwapBar>]) {
    let mut cur_day: Option<NaiveDate> = None;
    let mut sum_w = 0.0_f64;
    let mut sum_wp = 0.0_f64;
    let mut sum_wp2 = 0.0_f64;
    for (i, c) in candles.iter().enumerate() {
        let d = c.close_time.date_naive();
        if cur_day != Some(d) {
            cur_day = Some(d);
            sum_w = 0.0;
            sum_wp = 0.0;
            sum_wp2 = 0.0;
        }
        let w = c.volume;
        let p = typical(c);
        if include_current {
            sum_w += w;
            sum_wp += w * p;
            sum_wp2 += w * p * p;
            out[i] = bands_from_sums(sum_w, sum_wp, sum_wp2);
        } else {
            out[i] = bands_from_sums(sum_w, sum_wp, sum_wp2);
            sum_w += w;
            sum_wp += w * p;
            sum_wp2 += w * p * p;
        }
    }
}

fn rolling_vwap(candles: &[Candle], rb: usize, include_current: bool, out: &mut [Option<VwapBar>]) {
    let n = candles.len();
    for (i, slot) in out.iter_mut().enumerate().take(n) {
        let end = if include_current {
            i
        } else {
            i.saturating_sub(1)
        };
        if end + 1 < rb {
            continue;
        }
        let start = end + 1 - rb;
        let mut sum_w = 0.0;
        let mut sum_wp = 0.0;
        let mut sum_wp2 = 0.0;
        for c in &candles[start..=end] {
            let w = c.volume;
            let p = typical(c);
            sum_w += w;
            sum_wp += w * p;
            sum_wp2 += w * p * p;
        }
        *slot = bands_from_sums(sum_w, sum_wp, sum_wp2);
    }
}
