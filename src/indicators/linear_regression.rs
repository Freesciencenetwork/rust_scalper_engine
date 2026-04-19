//! Rolling linear regression of `y` on x = 0..n-1; returns slope at the window end.

pub fn linear_regression_slope_series(y: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = y.len();
    let mut out = vec![None; n];
    if period < 2 || n < period {
        return out;
    }
    let xs: Vec<f64> = (0..period).map(|i| i as f64).collect();
    let mean_x = (period - 1) as f64 / 2.0;
    let var_x: f64 = xs
        .iter()
        .map(|x| {
            let d = x - mean_x;
            d * d
        })
        .sum();
    if var_x < f64::EPSILON {
        return out;
    }
    for i in period - 1..n {
        let w = &y[i + 1 - period..=i];
        let mean_y: f64 = w.iter().sum::<f64>() / period as f64;
        let mut cov = 0.0;
        for (j, &yv) in w.iter().enumerate() {
            let x = j as f64;
            cov += (x - mean_x) * (yv - mean_y);
        }
        out[i] = Some(cov / var_x);
    }
    out
}
