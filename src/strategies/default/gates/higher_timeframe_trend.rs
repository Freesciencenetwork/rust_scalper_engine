use crate::market_data::PreparedCandle;

pub fn passes(frame: &PreparedCandle) -> bool {
    matches!(
        (frame.ema_fast_higher, frame.ema_slow_higher),
        (Some(fast), Some(slow)) if fast > slow
    )
}
