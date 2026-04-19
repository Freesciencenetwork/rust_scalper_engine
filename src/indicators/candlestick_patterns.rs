//! Common single-bar / two-bar candlestick pattern flags (heuristic).

use crate::domain::Candle;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CandlestickPatternBar {
    pub bull_engulfing: bool,
    pub bear_engulfing: bool,
    pub hammer: bool,
    pub shooting_star: bool,
    pub doji: bool,
}

pub fn candlestick_pattern_series(candles: &[Candle]) -> Vec<CandlestickPatternBar> {
    let n = candles.len();
    let mut out: Vec<CandlestickPatternBar> =
        (0..n).map(|_| CandlestickPatternBar::default()).collect();
    if n < 2 {
        return out;
    }
    for i in 1..n {
        let c = &candles[i];
        let p = &candles[i - 1];
        let body = (c.close - c.open).abs();
        let range = (c.high - c.low).max(f64::EPSILON);
        let upper = c.high - c.open.max(c.close);
        let lower = c.open.min(c.close) - c.low;
        out[i].doji = body <= 0.1 * range;
        out[i].hammer = body < range * 0.35 && lower > 2.0 * body.max(1e-12) && upper < body;
        out[i].shooting_star = body < range * 0.35 && upper > 2.0 * body.max(1e-12) && lower < body;
        out[i].bull_engulfing =
            c.close > c.open && p.close < p.open && c.open <= p.close && c.close >= p.open;
        out[i].bear_engulfing =
            c.close < c.open && p.close > p.open && c.open >= p.close && c.close <= p.open;
    }
    out
}
