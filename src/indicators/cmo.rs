//! Chande momentum oscillator: 100 × (Su − Sd) / (Su + Sd) over `period`.

pub fn cmo_series(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out = vec![None; n];
    if period == 0 || n < period + 1 {
        return out;
    }
    for (i, slot) in out.iter_mut().enumerate().take(n).skip(period) {
        let mut su = 0.0_f64;
        let mut sd = 0.0_f64;
        for j in i + 1 - period..=i {
            let ch = closes[j] - closes[j - 1];
            if ch > 0.0 {
                su += ch;
            } else {
                sd += -ch;
            }
        }
        let den = su + sd;
        *slot = Some(if den < f64::EPSILON {
            0.0
        } else {
            100.0 * (su - sd) / den
        });
    }
    out
}
