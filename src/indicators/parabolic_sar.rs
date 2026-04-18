//! Parabolic SAR (Wilder-style step acceleration).

#[derive(Clone, Debug, PartialEq)]
pub struct ParabolicSarBar {
    pub sar: f64,
    pub is_long: bool,
}

/// `af_start` / `af_step` / `af_max` commonly `0.02` / `0.02` / `0.20`.
pub fn parabolic_sar_series(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    af_start: f64,
    af_step: f64,
    af_max: f64,
) -> Vec<Option<ParabolicSarBar>> {
    let n = highs.len().min(lows.len()).min(closes.len());
    let mut out = vec![None; n];
    if n < 2 {
        return out;
    }
    let mut is_long = closes[1] >= closes[0];
    let mut af = af_start;
    let mut ep = if is_long {
        highs[0].max(highs[1])
    } else {
        lows[0].min(lows[1])
    };
    let mut sar = if is_long {
        lows[0].min(lows[1])
    } else {
        highs[0].max(highs[1])
    };
    out[0] = Some(ParabolicSarBar { sar, is_long });
    out[1] = Some(ParabolicSarBar { sar, is_long });
    for i in 2..n {
        let prev_sar = sar;
        if is_long {
            sar = prev_sar + af * (ep - prev_sar);
            sar = sar.min(lows[i - 1]).min(lows[i - 2]);
            if lows[i] < sar {
                is_long = false;
                sar = ep;
                ep = lows[i];
                af = af_start;
            } else if highs[i] > ep {
                ep = highs[i];
                af = (af + af_step).min(af_max);
            }
        } else {
            sar = prev_sar + af * (ep - prev_sar);
            sar = sar.max(highs[i - 1]).max(highs[i - 2]);
            if highs[i] > sar {
                is_long = true;
                sar = ep;
                ep = highs[i];
                af = af_start;
            } else if lows[i] < ep {
                ep = lows[i];
                af = (af + af_step).min(af_max);
            }
        }
        out[i] = Some(ParabolicSarBar { sar, is_long });
    }
    out
}
