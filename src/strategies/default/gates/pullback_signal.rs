use crate::market_data::PreparedCandle;

pub fn passes(frame: &PreparedCandle) -> bool {
    matches!(
        (frame.ema_fast_15m, frame.ema_slow_15m),
        (Some(ema_fast), Some(ema_slow))
            if frame.candle.low <= ema_fast
                && frame.candle.close > ema_fast
                && frame.candle.close >= ema_slow
    )
}
