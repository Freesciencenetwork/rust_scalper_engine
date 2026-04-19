//! Ordinary least squares for simple linear regression *y* ≈ *a* + *b* *x*.

use super::descriptive::mean;

#[derive(Clone, Debug, PartialEq)]
pub struct OlsFit {
    pub n: usize,
    pub slope: f64,
    pub intercept: f64,
    /// Coefficient of determination R² on the training sample.
    pub r_squared: f64,
}

/// OLS of *y* on *x*; `None` if lengths differ, *n* < 2, or Var(*x*) = 0.
pub fn ols_simple(x: &[f64], y: &[f64]) -> Option<OlsFit> {
    let n = x.len();
    if n != y.len() || n < 2 {
        return None;
    }
    let mx = mean(x)?;
    let my = mean(y)?;
    let mut sxx = 0.0_f64;
    let mut sxy = 0.0_f64;
    for i in 0..n {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        sxx += dx * dx;
        sxy += dx * dy;
    }
    if sxx < f64::EPSILON {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let mut ss_tot = 0.0_f64;
    let mut ss_res = 0.0_f64;
    for i in 0..n {
        let pred = intercept + slope * x[i];
        let e = y[i] - pred;
        ss_res += e * e;
        let d = y[i] - my;
        ss_tot += d * d;
    }
    let r_squared = if ss_tot < f64::EPSILON {
        1.0
    } else {
        1.0 - ss_res / ss_tot
    };
    Some(OlsFit {
        n,
        slope,
        intercept,
        r_squared,
    })
}
