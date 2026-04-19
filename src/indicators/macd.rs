//! MACD = EMA(fast) − EMA(slow); signal = EMA(signal_period) of MACD; hist = MACD − signal.

use super::ema_series;

#[derive(Clone, Debug, PartialEq)]
pub struct MacdBar {
    pub line: f64,
    pub signal: f64,
    pub hist: f64,
}

/// Classic 12 / 26 / 9 MACD. `None` until `slow + signal_p` bars (warm-up).
pub fn macd_series(
    closes: &[f64],
    fast: usize,
    slow: usize,
    signal_p: usize,
) -> Vec<Option<MacdBar>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if fast == 0 || slow == 0 || signal_p == 0 || n == 0 {
        return out;
    }
    let ema_fast = ema_series(closes, fast);
    let ema_slow = ema_series(closes, slow);
    let macd_line: Vec<f64> = ema_fast
        .iter()
        .zip(ema_slow.iter())
        .map(|(a, b)| a - b)
        .collect();
    let signal_series = ema_series(&macd_line, signal_p);
    let warmup = slow + signal_p;
    for i in 0..n {
        if i + 1 < warmup {
            continue;
        }
        let sig = signal_series[i];
        let m = macd_line[i];
        out[i] = Some(MacdBar {
            line: m,
            signal: sig,
            hist: m - sig,
        });
    }
    out
}
