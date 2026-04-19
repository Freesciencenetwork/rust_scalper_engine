//! Basic moments: mean, weighted mean, variance, standard deviation.

/// Arithmetic mean; `None` if `values` is empty.
pub fn mean(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n == 0 {
        return None;
    }
    Some(values.iter().sum::<f64>() / n as f64)
}

/// `None` if lengths differ, `values` empty, or sum of weights is zero / non-finite.
pub fn weighted_mean(values: &[f64], weights: &[f64]) -> Option<f64> {
    if values.len() != weights.len() || values.is_empty() {
        return None;
    }
    let mut num = 0.0_f64;
    let mut den = 0.0_f64;
    for (&v, &w) in values.iter().zip(weights.iter()) {
        if !w.is_finite() || !v.is_finite() {
            return None;
        }
        num += v * w;
        den += w;
    }
    if den.abs() < f64::EPSILON {
        None
    } else {
        Some(num / den)
    }
}

/// Population variance σ² = E[(X−μ)²]; `None` if empty.
pub fn population_variance(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n == 0 {
        return None;
    }
    let mu = mean(values)?;
    let s: f64 = values
        .iter()
        .map(|x| {
            let d = x - mu;
            d * d
        })
        .sum();
    Some(s / n as f64)
}

/// Unbiased sample variance (Bessel) with divisor *n*−1; `None` if *n* < 2.
pub fn sample_variance(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 2 {
        return None;
    }
    let mu = mean(values)?;
    let s: f64 = values
        .iter()
        .map(|x| {
            let d = x - mu;
            d * d
        })
        .sum();
    Some(s / (n - 1) as f64)
}

pub fn population_std(values: &[f64]) -> Option<f64> {
    population_variance(values).map(|v| v.sqrt())
}

pub fn sample_std(values: &[f64]) -> Option<f64> {
    sample_variance(values).map(|v| v.sqrt())
}

/// Standard error of the mean: *s* / √*n* using sample standard deviation; `None` if *n* < 2.
pub fn standard_error_mean(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 2 {
        return None;
    }
    let s = sample_std(values)?;
    Some(s / (n as f64).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_and_sample_var_match_hand_calc() {
        let x = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((mean(&x).unwrap() - 5.0).abs() < 1e-9);
        let v = sample_variance(&x).unwrap();
        assert!((v - 4.571428571428571).abs() < 1e-6);
    }
}
