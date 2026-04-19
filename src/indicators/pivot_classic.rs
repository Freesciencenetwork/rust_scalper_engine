//! Classic daily pivots from the **prior** UTC calendar day H/L/C.
//!
//! **Timeframe notes:**
//! - Sub-daily bars (1m–4h): aggregates all bars in a UTC day and uses the prior day's H/L/C. ✓
//! - Daily bars: each bar is one UTC day; uses the prior bar's H/L/C. ✓
//! - Weekly+ bars: `pred_opt()` on the bar's date gives a day not present in the map →
//!   all values are `None`. Pivots are not meaningful at that resolution.

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::Candle;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PivotClassicBar {
    pub pivot_p: Option<f64>,
    pub pivot_r1: Option<f64>,
    pub pivot_r2: Option<f64>,
    pub pivot_r3: Option<f64>,
    pub pivot_s1: Option<f64>,
    pub pivot_s2: Option<f64>,
    pub pivot_s3: Option<f64>,
}

fn classic_from_hlc(h: f64, l: f64, c: f64) -> PivotClassicBar {
    let p = (h + l + c) / 3.0;
    let r1 = 2.0 * p - l;
    let s1 = 2.0 * p - h;
    let r2 = p + (h - l);
    let s2 = p - (h - l);
    let r3 = h + 2.0 * (p - l);
    let s3 = l - 2.0 * (h - p);
    PivotClassicBar {
        pivot_p: Some(p),
        pivot_r1: Some(r1),
        pivot_r2: Some(r2),
        pivot_r3: Some(r3),
        pivot_s1: Some(s1),
        pivot_s2: Some(s2),
        pivot_s3: Some(s3),
    }
}

/// `out[i]` uses the last fully observed UTC day **before** `candles[i].close_time.date_naive()`.
pub fn pivot_classic_series(candles: &[Candle]) -> Vec<PivotClassicBar> {
    let n = candles.len();
    let mut out = vec![PivotClassicBar::default(); n];
    if n == 0 {
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
            out[i] = classic_from_hlc(h, l, cl);
        }
    }
    out
}
