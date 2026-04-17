use crate::strategy::data::PreparedDataset;
use crate::strategy::state::FailedAcceptanceState;

pub fn update(
    state: &mut FailedAcceptanceState,
    index: usize,
    dataset: &PreparedDataset,
    breakout_lookback: usize,
) {
    if index < breakout_lookback {
        return;
    }
    let start = index - breakout_lookback;
    let breakout_level = dataset.frames_15m[start..index]
        .iter()
        .map(|frame| frame.candle.high)
        .fold(f64::MIN, f64::max);
    let close = dataset.frames_15m[index].candle.close;

    if close > breakout_level {
        state.breakout_level = Some(breakout_level);
        state.active = false;
        return;
    }

    if let Some(level) = state.breakout_level {
        if close < level {
            state.active = true;
        } else if close > level {
            state.active = false;
        }
    }
}
