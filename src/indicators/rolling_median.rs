pub fn rolling_median(values: &[f64], lookback: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; values.len()];
    if lookback == 0 {
        return result;
    }

    for index in 0..values.len() {
        if index + 1 < lookback {
            continue;
        }
        let start = index + 1 - lookback;
        let mut window = values[start..=index].to_vec();
        window.sort_by(f64::total_cmp);
        let mid = window.len() / 2;
        let median = if window.len().is_multiple_of(2) {
            (window[mid - 1] + window[mid]) / 2.0
        } else {
            window[mid]
        };
        result[index] = Some(median);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::rolling_median;

    #[test]
    fn returns_none_series_for_zero_lookback() {
        assert_eq!(rolling_median(&[1.0, 2.0], 0), vec![None, None]);
    }

    #[test]
    fn computes_even_window_median() {
        let median = rolling_median(&[4.0, 1.0, 3.0, 2.0], 4);
        assert_eq!(median, vec![None, None, None, Some(2.5)]);
    }
}
