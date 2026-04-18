//! Ichimoku cloud (9 / 26 / 52, +26 forward displacement on spans).

use crate::domain::Candle;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IchimokuBar {
    pub tenkan_9: Option<f64>,
    pub kijun_26: Option<f64>,
    pub senkou_a_26: Option<f64>,
    pub senkou_b_52: Option<f64>,
    /// Source close for the lagging line (rendering applies −26 on charts).
    pub chikou_close_shifted: Option<f64>,
}

fn hl_mid(candles: &[Candle], end: usize, len: usize) -> Option<f64> {
    if end + 1 < len {
        return None;
    }
    let s = end + 1 - len;
    let mut hh = f64::NEG_INFINITY;
    let mut ll = f64::INFINITY;
    for c in &candles[s..=end] {
        hh = hh.max(c.high);
        ll = ll.min(c.low);
    }
    Some((hh + ll) / 2.0)
}

pub fn ichimoku_series(candles: &[Candle]) -> Vec<IchimokuBar> {
    let n = candles.len();
    let mut out: Vec<IchimokuBar> = (0..n).map(|_| IchimokuBar::default()).collect();
    for i in 0..n {
        out[i].tenkan_9 = hl_mid(candles, i, 9);
        out[i].kijun_26 = hl_mid(candles, i, 26);
        out[i].chikou_close_shifted = Some(candles[i].close);
        let j = i as isize - 26;
        if j >= 0 {
            let j = j as usize;
            let t = out[j].tenkan_9;
            let k = out[j].kijun_26;
            out[i].senkou_a_26 = match (t, k) {
                (Some(a), Some(b)) => Some((a + b) / 2.0),
                _ => None,
            };
            // 52-bar midpoint ending at `j` needs `j >= 51` so the window starts at 0.
            if j >= 51 {
                out[i].senkou_b_52 = hl_mid(candles, j, 52);
            }
        }
    }
    out
}
