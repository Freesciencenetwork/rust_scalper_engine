//! Bollinger bands: middle = SMA(n); width = k·population stdev of closes.

use super::sma_series;

#[derive(Clone, Debug, PartialEq)]
pub struct BollingerBar {
    pub middle: f64,
    pub upper: f64,
    pub lower: f64,
}

fn stdev_population(window: &[f64], mean: f64) -> f64 {
    if window.is_empty() {
        return 0.0;
    }
    let v: f64 = window
        .iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / window.len() as f64;
    v.sqrt()
}

/// `k` typically `2.0`. Uses population standard deviation of closes in the window.
pub fn bollinger_series(closes: &[f64], period: usize, k: f64) -> Vec<Option<BollingerBar>> {
    let sma = sma_series(closes, period);
    let mut out = vec![None; closes.len()];
    for i in 0..closes.len() {
        let Some(mid) = sma[i] else { continue };
        if i + 1 < period {
            continue;
        }
        let w = &closes[i + 1 - period..=i];
        let sd = stdev_population(w, mid);
        out[i] = Some(BollingerBar {
            middle: mid,
            upper: mid + k * sd,
            lower: mid - k * sd,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::bollinger_series;

    #[test]
    fn bollinger_uses_population_standard_deviation() {
        let bands = bollinger_series(&[1.0, 2.0, 3.0], 3, 1.0);
        let last = bands[2].as_ref().expect("bands");
        let expected = (2.0_f64 / 3.0).sqrt();
        assert!((last.middle - 2.0).abs() < 1e-9);
        assert!((last.upper - (2.0 + expected)).abs() < 1e-9);
        assert!((last.lower - (2.0 - expected)).abs() < 1e-9);
    }
}
