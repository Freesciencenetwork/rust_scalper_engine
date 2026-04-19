//! Fibonacci daily pivots from prior UTC day H/L/C.
//!
//! **Timeframe notes:** identical to `pivot_classic` — works for sub-daily and daily bars;
//! returns `None` for all fields when bars span more than one UTC day (weekly+).

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::Candle;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PivotFibBar {
    pub pivot_p: Option<f64>,
    pub pivot_r1: Option<f64>,
    pub pivot_r2: Option<f64>,
    pub pivot_r3: Option<f64>,
    pub pivot_s1: Option<f64>,
    pub pivot_s2: Option<f64>,
    pub pivot_s3: Option<f64>,
}

fn fib_from_hlc(h: f64, l: f64, c: f64) -> PivotFibBar {
    let p = (h + l + c) / 3.0;
    let r = h - l;
    PivotFibBar {
        pivot_p: Some(p),
        pivot_r1: Some(p + 0.382 * r),
        pivot_r2: Some(p + 0.618 * r),
        pivot_r3: Some(p + 1.000 * r),
        pivot_s1: Some(p - 0.382 * r),
        pivot_s2: Some(p - 0.618 * r),
        pivot_s3: Some(p - 1.000 * r),
    }
}

pub fn pivot_fib_series(candles: &[Candle]) -> Vec<PivotFibBar> {
    let mut out = vec![PivotFibBar::default(); candles.len()];
    if candles.is_empty() {
        return out;
    }
    let mut daily: BTreeMap<NaiveDate, (f64, f64, f64)> = BTreeMap::new();
    for c in candles {
        let d = c.close_time.date_naive();
        daily
            .entry(d)
            .and_modify(|(hh, ll, cc)| {
                *hh = hh.max(c.high);
                *ll = ll.min(c.low);
                *cc = c.close;
            })
            .or_insert((c.high, c.low, c.close));
    }
    for (i, c) in candles.iter().enumerate() {
        let d = c.close_time.date_naive();
        let Some(prev_d) = d.pred_opt() else {
            continue;
        };
        if let Some(&(h, l, cl)) = daily.get(&prev_d) {
            out[i] = fib_from_hlc(h, l, cl);
        }
    }
    out
}
