//! Rolling OHLCV volume-at-price approximation: POC and value area (VAL / VAH).
//!
//! Each bar’s volume is spread linearly across its [low, high] range into fixed
//! price bins covering the window’s min(low)–max(high). POC is the bin with
//! the most volume; VAL/VAH bound the value area built by expanding from POC
//! toward adjacent higher-volume bins until `value_area_ratio` of window
//! volume is included (standard auction-market style).

use crate::domain::Candle;

#[derive(Clone, Debug, PartialEq)]
pub struct VolumeProfileZones {
    pub val: f64,
    pub poc: f64,
    pub vah: f64,
}

/// `end_index` inclusive; uses `candles[end_index + 1 - lookback ..= end_index]`.
pub fn volume_profile_zones(
    candles: &[Candle],
    end_index: usize,
    lookback: usize,
    bin_count: usize,
    value_area_ratio: f64,
) -> Option<VolumeProfileZones> {
    if lookback == 0 || bin_count < 2 || end_index + 1 < lookback {
        return None;
    }
    let ratio = value_area_ratio.clamp(1e-6, 1.0);
    let start = end_index + 1 - lookback;
    let window = candles.get(start..=end_index)?;

    let mut range_lo = f64::INFINITY;
    let mut range_hi = f64::NEG_INFINITY;
    for c in window {
        range_lo = range_lo.min(c.low).min(c.high);
        range_hi = range_hi.max(c.low).max(c.high);
    }
    if !range_lo.is_finite() || !range_hi.is_finite() {
        return None;
    }
    if range_hi < range_lo {
        std::mem::swap(&mut range_lo, &mut range_hi);
    }

    let width = {
        let w = (range_hi - range_lo) / bin_count as f64;
        if w <= f64::EPSILON * range_hi.abs().max(1.0) {
            (range_hi.abs().max(1.0)) * 1e-9
        } else {
            w
        }
    };
    // Tile bins exactly to a synthetic upper bound (may extend past raw window max).
    let bin_top = range_lo + width * bin_count as f64;

    let mut bins = vec![0.0_f64; bin_count];
    for c in window {
        accumulate_bar_volume(c, range_lo, width, bin_count, bin_top, &mut bins);
    }

    let total: f64 = bins.iter().sum();
    if total <= 0.0 || !total.is_finite() {
        return None;
    }

    let poc_idx = bins
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)?;

    let (left_idx, right_idx) = expand_value_area(&bins, poc_idx, total * ratio);
    let val = range_lo + left_idx as f64 * width;
    let vah = range_lo + (right_idx + 1) as f64 * width;
    let poc = range_lo + (poc_idx as f64 + 0.5) * width;

    Some(VolumeProfileZones { val, poc, vah })
}

fn accumulate_bar_volume(
    c: &Candle,
    range_lo: f64,
    width: f64,
    bin_count: usize,
    bin_top: f64,
    bins: &mut [f64],
) {
    let v = c.volume;
    if v <= 0.0 || !v.is_finite() {
        return;
    }
    let mut lo = c.low.min(c.high);
    let mut hi = c.low.max(c.high);
    if !lo.is_finite() || !hi.is_finite() {
        return;
    }
    if hi - lo <= f64::EPSILON * hi.abs().max(1.0) {
        let idx = price_to_bin_index((lo + hi) * 0.5, range_lo, width, bin_count);
        bins[idx] += v;
        return;
    }
    if lo < range_lo {
        lo = range_lo;
    }
    if hi > bin_top {
        hi = bin_top;
    }
    if hi <= lo {
        return;
    }
    let bar_range = hi - lo;
    for i in 0..bin_count {
        let b_lo = range_lo + i as f64 * width;
        let b_hi = b_lo + width;
        let overlap = (hi.min(b_hi) - lo.max(b_lo)).max(0.0);
        if overlap > 0.0 {
            bins[i] += v * overlap / bar_range;
        }
    }
}

fn price_to_bin_index(price: f64, range_lo: f64, width: f64, bin_count: usize) -> usize {
    if width <= 0.0 {
        return 0;
    }
    let mut i = ((price - range_lo) / width).floor() as isize;
    if i < 0 {
        i = 0;
    }
    if i >= bin_count as isize {
        i = bin_count as isize - 1;
    }
    i as usize
}

/// Expand from `poc_idx` by repeatedly adding the adjacent bin with larger volume
/// until included volume >= `target_volume`.
fn expand_value_area(bins: &[f64], poc_idx: usize, target_volume: f64) -> (usize, usize) {
    let mut left = poc_idx;
    let mut right = poc_idx;
    let mut included = bins[poc_idx];
    while included < target_volume && (left > 0 || right + 1 < bins.len()) {
        let left_vol = if left > 0 { Some(bins[left - 1]) } else { None };
        let right_vol = if right + 1 < bins.len() {
            Some(bins[right + 1])
        } else {
            None
        };
        match (left_vol, right_vol) {
            (Some(lv), Some(rv)) => {
                if lv >= rv {
                    left -= 1;
                    included += lv;
                } else {
                    right += 1;
                    included += rv;
                }
            }
            (Some(lv), None) => {
                left -= 1;
                included += lv;
            }
            (None, Some(rv)) => {
                right += 1;
                included += rv;
            }
            (None, None) => break,
        }
    }
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn c(t_min: i64, open: f64, high: f64, low: f64, close: f64, vol: f64) -> Candle {
        let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        Candle {
            close_time: base + Duration::minutes(t_min),
            open,
            high,
            low,
            close,
            volume: vol,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        }
    }

    #[test]
    fn flat_window_poc_near_center_of_mass() {
        let candles: Vec<Candle> = (0..8)
            .map(|i| c(i, 100.0, 102.0, 99.0, 101.0, 10.0 + i as f64))
            .collect();
        let z = volume_profile_zones(&candles, 7, 8, 24, 0.7).expect("zones");
        assert!(z.val <= z.poc && z.poc <= z.vah);
        assert!(z.vah - z.val > 0.0);
    }

    #[test]
    fn concentrated_low_prints_draw_poc_downward() {
        let candles = vec![
            c(0, 50.0, 51.0, 50.0, 50.5, 1000.0),
            c(1, 50.0, 51.0, 50.0, 50.5, 1000.0),
            c(2, 110.0, 111.0, 109.0, 110.0, 1.0),
            c(3, 110.0, 111.0, 109.0, 110.0, 1.0),
        ];
        let z = volume_profile_zones(&candles, 3, 4, 32, 0.7).expect("zones");
        assert!(z.poc < 75.0, "poc should sit with dense low prints, got {}", z.poc);
        assert!(z.val <= z.poc && z.poc <= z.vah);
    }
}
