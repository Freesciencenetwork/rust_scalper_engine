use crate::market_data::PreparedCandle;

pub fn passes(frame: &PreparedCandle) -> bool {
    matches!(
        (frame.ema_fast_1h, frame.ema_slow_1h),
        (Some(fast), Some(slow)) if fast > slow
    )
}
