use crate::market_data::PreparedCandle;

pub fn passes(frame: &PreparedCandle) -> bool {
    matches!(
        (frame.ema_fast, frame.ema_slow),
        (Some(ema_fast), Some(ema_slow))
            if frame.candle.low <= ema_fast
                && frame.candle.close > ema_fast
                && frame.candle.close >= ema_slow
    )
}
