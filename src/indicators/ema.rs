pub fn ema_series(values: &[f64], period: usize) -> Vec<f64> {
    assert!(period > 0, "period must be positive");
    if values.is_empty() {
        return Vec::new();
    }

    let alpha = 2.0 / (period as f64 + 1.0);
    let mut result = Vec::with_capacity(values.len());
    let mut prev = values[0];
    result.push(prev);

    for &value in &values[1..] {
        prev = alpha * value + (1.0 - alpha) * prev;
        result.push(prev);
    }

    result
}

pub fn ema_seeded_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    assert!(period > 0, "period must be positive");
    let mut result = vec![None; values.len()];
    if values.len() < period {
        return result;
    }

    let alpha = 2.0 / (period as f64 + 1.0);
    let mut prev = values[..period].iter().sum::<f64>() / period as f64;
    result[period - 1] = Some(prev);

    for index in period..values.len() {
        prev = alpha * values[index] + (1.0 - alpha) * prev;
        result[index] = Some(prev);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::{ema_seeded_series, ema_series};

    #[test]
    fn ema_series_tracks_values() {
        let ema = ema_series(&[1.0, 2.0, 3.0], 2);
        assert_eq!(ema.len(), 3);
        assert!((ema[0] - 1.0).abs() < 1e-9);
        assert!(ema[2] > ema[1]);
    }

    #[test]
    fn seeded_ema_starts_from_initial_sma() {
        let ema = ema_seeded_series(&[1.0, 2.0, 3.0], 2);
        assert_eq!(ema[0], None);
        assert!((ema[1].expect("seeded ema") - 1.5).abs() < 1e-9);
        assert!((ema[2].expect("next ema") - 2.5).abs() < 1e-9);
    }
}
