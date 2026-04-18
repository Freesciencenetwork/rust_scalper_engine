/// Simple moving average; values before `period-1` are `None`.
pub fn sma_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    if period == 0 {
        return vec![None; values.len()];
    }
    let mut out = Vec::with_capacity(values.len());
    for i in 0..values.len() {
        if i + 1 < period {
            out.push(None);
            continue;
        }
        let s: f64 = values[i + 1 - period..=i].iter().sum();
        out.push(Some(s / period as f64));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_three_period() {
        let v = vec![1.0, 2.0, 3.0, 4.0];
        let s = sma_series(&v, 3);
        assert_eq!(s[0], None);
        assert_eq!(s[1], None);
        assert!((s[2].unwrap() - 2.0).abs() < 1e-9);
        assert!((s[3].unwrap() - 3.0).abs() < 1e-9);
    }
}
