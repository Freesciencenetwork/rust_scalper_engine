//! Relative strength index (Wilder / RMA smoothing), TradingView-style.

/// RSI over `period` (commonly 14). Uses Wilder smoothing for avg gain/loss.
pub fn rsi_series(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = closes.len();
    let mut out: Vec<Option<f64>> = (0..n).map(|_| None).collect();
    if period == 0 || n < period + 1 {
        return out;
    }
    let mut gains = Vec::with_capacity(n);
    let mut losses = Vec::with_capacity(closes.len());
    gains.push(0.0);
    losses.push(0.0);
    for i in 1..closes.len() {
        let ch = closes[i] - closes[i - 1];
        gains.push(ch.max(0.0));
        losses.push((-ch).max(0.0));
    }

    let mut avg_gain: f64 = gains[1..=period].iter().sum::<f64>() / period as f64;
    let mut avg_loss: f64 = losses[1..=period].iter().sum::<f64>() / period as f64;

    let rs = if avg_loss == 0.0 {
        f64::INFINITY
    } else {
        avg_gain / avg_loss
    };
    out[period] = Some(100.0 - (100.0 / (1.0 + rs)));

    for i in (period + 1)..closes.len() {
        avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;
        let rs = if avg_loss == 0.0 {
            f64::INFINITY
        } else {
            avg_gain / avg_loss
        };
        out[i] = Some(100.0 - (100.0 / (1.0 + rs)));
    }
    out
}
