//! Higher moments: skewness and excess kurtosis (sample, bias-adjusted).

use super::descriptive::{mean, sample_std};

/// Fisher–Pearson sample skewness; `None` if *n* < 3 or sample stdev is zero.
pub fn sample_skewness(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 3 {
        return None;
    }
    let m = mean(values)?;
    let s = sample_std(values)?;
    if s.abs() < f64::EPSILON {
        return None;
    }
    let mut m3 = 0.0_f64;
    for &x in values {
        let z = (x - m) / s;
        m3 += z * z * z;
    }
    let factor = n as f64 / ((n - 1) as f64 * (n - 2) as f64);
    Some(factor * m3)
}

/// Sample excess kurtosis (Fisher); `None` if *n* < 4 or sample stdev is zero.
pub fn sample_excess_kurtosis(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 4 {
        return None;
    }
    let m = mean(values)?;
    let s = sample_std(values)?;
    if s.abs() < f64::EPSILON {
        return None;
    }
    let mut m4 = 0.0_f64;
    for &x in values {
        let z = (x - m) / s;
        m4 += z * z * z * z;
    }
    let nf = n as f64;
    let num = nf * (nf + 1.0) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0)) * m4;
    let den = 3.0 * (nf - 1.0).powi(2) / ((nf - 2.0) * (nf - 3.0));
    Some(num - den)
}
