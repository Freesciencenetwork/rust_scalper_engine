//! Covariance and correlation (Pearson, Spearman).

use super::descriptive::mean;
use super::rank::rank_average;

/// Sample covariance with divisor *n*−1; `None` if lengths differ or *n* < 2.
pub fn sample_covariance(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len();
    if n != y.len() || n < 2 {
        return None;
    }
    let mx = mean(x)?;
    let my = mean(y)?;
    let mut s = 0.0_f64;
    for i in 0..n {
        s += (x[i] - mx) * (y[i] - my);
    }
    Some(s / (n - 1) as f64)
}

/// Pearson product-moment *r*; `None` if undefined (zero variance on either side).
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len();
    if n != y.len() || n < 2 {
        return None;
    }
    let cov = sample_covariance(x, y)?;
    let sx = super::descriptive::sample_std(x)?;
    let sy = super::descriptive::sample_std(y)?;
    let d = sx * sy;
    if d.abs() < f64::EPSILON {
        None
    } else {
        Some(cov / d)
    }
}

/// Spearman ρ: Pearson correlation of average ranks (ties split).
pub fn spearman_correlation(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }
    let rx = rank_average(x)?;
    let ry = rank_average(y)?;
    pearson_correlation(&rx, &ry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pearson_perfect_positive() {
        let x = [1.0, 2.0, 3.0, 4.0];
        let y = [2.0, 4.0, 6.0, 8.0];
        let r = pearson_correlation(&x, &y).unwrap();
        assert!((r - 1.0).abs() < 1e-9);
    }
}
