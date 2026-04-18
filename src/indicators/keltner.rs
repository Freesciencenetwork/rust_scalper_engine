//! Keltner channels: middle = EMA(close); bands = middle ± mult × ATR.

use crate::domain::Candle;

use super::{atr_series, ema_series};

#[derive(Clone, Debug, PartialEq)]
pub struct KeltnerBar {
    pub middle: f64,
    pub upper: f64,
    pub lower: f64,
}

pub fn keltner_series(
    candles: &[Candle],
    ema_period: usize,
    atr_period: usize,
    atr_mult: f64,
) -> Vec<Option<KeltnerBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if ema_period == 0 || atr_period == 0 || n == 0 {
        return out;
    }
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let ema_c = ema_series(&closes, ema_period);
    let atr = atr_series(candles, atr_period);
    let warmup = ema_period.max(atr_period);
    for i in 0..n {
        if i + 1 < warmup {
            continue;
        }
        let Some(a) = atr[i] else { continue };
        let mid = ema_c[i];
        out[i] = Some(KeltnerBar {
            middle: mid,
            upper: mid + atr_mult * a,
            lower: mid - atr_mult * a,
        });
    }
    out
}
