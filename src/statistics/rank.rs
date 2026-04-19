//! Order statistics: median, quantiles, linear percentile, IQR, average ranks.

/// Average ranks (1-based) for `values`, with ties receiving the mean of their rank positions.
pub fn rank_average(values: &[f64]) -> Option<Vec<f64>> {
    let n = values.len();
    if n == 0 {
        return None;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| {
        values[i]
            .partial_cmp(&values[j])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0.0_f64; n];
    let mut start = 0usize;
    while start < n {
        let mut end = start;
        while end + 1 < n && values[order[end + 1]] == values[order[start]] {
            end += 1;
        }
        let avg = ((start + 1) + (end + 1)) as f64 / 2.0;
        for k in start..=end {
            ranks[order[k]] = avg;
        }
        start = end + 1;
    }
    Some(ranks)
}

/// Median; averages two middle values when *n* is even. `None` if empty.
pub fn median(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n == 0 {
        return None;
    }
    let mut v: Vec<f64> = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if n % 2 == 1 {
        Some(v[n / 2])
    } else {
        Some((v[n / 2 - 1] + v[n / 2]) / 2.0)
    }
}

/// Linearly interpolated percentile in \[0, 100\], R type‑7 style (same family as NumPy `method="linear"` default).
/// Uses the empirical CDF position *h* = (*p*/100)·(*n*−1) between sorted order statistics.
pub fn percentile_linear(values: &[f64], p: f64) -> Option<f64> {
    if !(0.0..=100.0).contains(&p) || values.is_empty() {
        return None;
    }
    let n = values.len();
    if n == 1 {
        return Some(values[0]);
    }
    let mut v: Vec<f64> = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let h = (p / 100.0) * (n - 1) as f64;
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    if lo == hi {
        return Some(v[lo]);
    }
    let w = h - lo as f64;
    Some(v[lo] * (1.0 - w) + v[hi] * w)
}

/// First and third quartiles and IQR = Q3 − Q1.
pub fn quartiles(values: &[f64]) -> Option<(f64, f64)> {
    let q1 = percentile_linear(values, 25.0)?;
    let q3 = percentile_linear(values, 75.0)?;
    Some((q1, q3))
}

pub fn interquartile_range(values: &[f64]) -> Option<f64> {
    let (q1, q3) = quartiles(values)?;
    Some(q3 - q1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_even_averages_middle_pair() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
    }

    #[test]
    fn percentile_endpoints() {
        let v = [10.0, 20.0, 30.0];
        assert!((percentile_linear(&v, 0.0).unwrap() - 10.0).abs() < 1e-9);
        assert!((percentile_linear(&v, 100.0).unwrap() - 30.0).abs() < 1e-9);
    }
}
