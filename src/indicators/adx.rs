//! Wilder-style ADX and directional indicators (+DI / −DI).

use crate::domain::Candle;

#[derive(Clone, Debug, PartialEq)]
pub struct AdxBar {
    pub adx: f64,
    pub di_plus: f64,
    pub di_minus: f64,
}

fn wilder_smooth(prev: f64, value: f64, period: usize) -> f64 {
    (prev * (period - 1) as f64 + value) / period as f64
}

/// `period` commonly 14. First valid output after `2*period` bars (conservative).
pub fn adx_series(candles: &[Candle], period: usize) -> Vec<Option<AdxBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    if period == 0 || n < period + 1 {
        return out;
    }

    let mut tr = vec![0.0_f64; n];
    let mut dmp = vec![0.0_f64; n];
    let mut dmm = vec![0.0_f64; n];

    for i in 1..n {
        let c = &candles[i];
        let p = &candles[i - 1];
        let tr_raw = (c.high - c.low)
            .max((c.high - p.close).abs())
            .max((c.low - p.close).abs());
        tr[i] = tr_raw;

        let up = c.high - p.high;
        let down = p.low - c.low;
        if up > down && up > 0.0 {
            dmp[i] = up;
        }
        if down > up && down > 0.0 {
            dmm[i] = down;
        }
    }

    let mut atr_sm = vec![0.0_f64; n];
    let mut dmp_sm = vec![0.0_f64; n];
    let mut dmm_sm = vec![0.0_f64; n];

    let first_sum: f64 = tr[1..=period].iter().sum();
    atr_sm[period] = first_sum;
    dmp_sm[period] = dmp[1..=period].iter().sum();
    dmm_sm[period] = dmm[1..=period].iter().sum();

    for i in (period + 1)..n {
        atr_sm[i] = wilder_smooth(atr_sm[i - 1], tr[i], period);
        dmp_sm[i] = wilder_smooth(dmp_sm[i - 1], dmp[i], period);
        dmm_sm[i] = wilder_smooth(dmm_sm[i - 1], dmm[i], period);
    }

    let mut dx = vec![0.0_f64; n];
    for i in period..n {
        if atr_sm[i] < f64::EPSILON {
            dx[i] = 0.0;
            continue;
        }
        let di_p = 100.0 * dmp_sm[i] / atr_sm[i];
        let di_m = 100.0 * dmm_sm[i] / atr_sm[i];
        let denom = di_p + di_m;
        dx[i] = if denom < f64::EPSILON {
            0.0
        } else {
            100.0 * (di_p - di_m).abs() / denom
        };
    }

    let mut adx_sm = vec![0.0_f64; n];
    let start = period * 2 - 1;
    if start < n {
        let first_adx: f64 = dx[period..=start].iter().sum::<f64>() / period as f64;
        adx_sm[start] = first_adx;
        for i in (start + 1)..n {
            adx_sm[i] = wilder_smooth(adx_sm[i - 1], dx[i], period);
        }
    }

    for i in start..n {
        if atr_sm[i] < f64::EPSILON {
            continue;
        }
        let di_p = 100.0 * dmp_sm[i] / atr_sm[i];
        let di_m = 100.0 * dmm_sm[i] / atr_sm[i];
        out[i] = Some(AdxBar {
            adx: adx_sm[i],
            di_plus: di_p,
            di_minus: di_m,
        });
    }
    out
}
