pub fn rolling_median(values: &[f64], lookback: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; values.len()];

    for index in 0..values.len() {
        if index + 1 < lookback {
            continue;
        }
        let start = index + 1 - lookback;
        let mut window = values[start..=index].to_vec();
        window.sort_by(f64::total_cmp);
        let mid = window.len() / 2;
        let median = if window.len() % 2 == 0 {
            (window[mid - 1] + window[mid]) / 2.0
        } else {
            window[mid]
        };
        result[index] = Some(median);
    }

    result
}
