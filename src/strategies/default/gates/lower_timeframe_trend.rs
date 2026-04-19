use crate::market_data::PreparedCandle;

pub fn passes(index: usize, frames: &[PreparedCandle], required_bars: usize) -> bool {
    if index + 1 < required_bars {
        return false;
    }
    let start = index + 1 - required_bars;
    frames[start..=index].iter().all(|frame| {
        matches!(
            (frame.ema_fast, frame.ema_slow),
            (Some(fast), Some(slow)) if fast > slow
        )
    })
}
