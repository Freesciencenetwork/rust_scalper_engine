//! Klinger volume oscillator: EMA(fast) of Volume Force − EMA(slow) of Volume Force.

use crate::domain::Candle;

use super::ema_series;

pub fn kvo_series(
    candles: &[Candle],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<Option<f64>>, Vec<Option<f64>>) {
    let n = candles.len();
    let mut kvo = vec![None; n];
    let mut kvo_sig = vec![None; n];
    if fast == 0 || slow == 0 || signal == 0 || n < 2 {
        return (kvo, kvo_sig);
    }
    let mut vf = vec![0.0_f64; n];
    let mut prev_trend = 0.0_f64;
    let mut prev_dm = 0.0_f64;
    let mut prev_cm = 0.0_f64;

    for i in 0..n {
        let dm = candles[i].high - candles[i].low;
        if i == 0 {
            prev_dm = dm;
            prev_cm = dm;
            continue;
        }

        let cur_hlc = candles[i].high + candles[i].low + candles[i].close;
        let prev_hlc = candles[i - 1].high + candles[i - 1].low + candles[i - 1].close;
        let trend = if cur_hlc > prev_hlc {
            1.0
        } else if cur_hlc < prev_hlc {
            -1.0
        } else {
            prev_trend
        };

        let cm = if (trend - prev_trend).abs() < f64::EPSILON {
            prev_cm + dm
        } else {
            prev_dm + dm
        };

        if cm.abs() >= f64::EPSILON {
            vf[i] = candles[i].volume * (2.0 * ((dm / cm) - 1.0)) * trend * 100.0;
        }

        prev_trend = trend;
        prev_dm = dm;
        prev_cm = cm;
    }
    let ema_f = ema_series(&vf, fast);
    let ema_s = ema_series(&vf, slow);
    let mut line = vec![0.0_f64; n];
    for i in 0..n {
        line[i] = ema_f[i] - ema_s[i];
    }
    let sig = ema_series(&line, signal);
    let warmup = slow.max(fast) + signal;
    for i in 0..n {
        if i + 1 < warmup {
            continue;
        }
        kvo[i] = Some(line[i]);
        kvo_sig[i] = Some(sig[i]);
    }
    (kvo, kvo_sig)
}
