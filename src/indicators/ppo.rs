//! Percentage price oscillator: `(EMA_fast − EMA_slow) / EMA_slow × 100`, signal EMA on line.

use super::ema_series;

#[derive(Clone, Debug, PartialEq)]
pub struct PpoBar {
    pub line: f64,
    pub signal: f64,
    pub hist: f64,
}

/// Classic 12 / 26 / 9 on **percentage** scale (distinct from MACD price scale).
pub fn ppo_series(
    closes: &[f64],
    fast: usize,
    slow: usize,
    signal_p: usize,
) -> Vec<Option<PpoBar>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if fast == 0 || slow == 0 || signal_p == 0 || n == 0 {
        return out;
    }
    let ema_fast = ema_series(closes, fast);
    let ema_slow = ema_series(closes, slow);
    let mut line = vec![0.0_f64; n];
    for i in 0..n {
        let s = ema_slow[i];
        line[i] = if s.abs() < f64::EPSILON {
            0.0
        } else {
            (ema_fast[i] - s) / s * 100.0
        };
    }
    let signal_series = ema_series(&line, signal_p);
    let warmup = slow + signal_p;
    for i in 0..n {
        if i + 1 < warmup {
            continue;
        }
        let sig = signal_series[i];
        let l = line[i];
        out[i] = Some(PpoBar {
            line: l,
            signal: sig,
            hist: l - sig,
        });
    }
    out
}
