use crate::config::StrategyConfig;
use crate::strategy::data::PreparedDataset;

pub fn active(
    index: usize,
    dataset: &PreparedDataset,
    entry_price: f64,
    config: &StrategyConfig,
) -> bool {
    let Some(atr) = dataset.frames_15m[index].atr_15m else {
        return false;
    };
    if index < 4 {
        return false;
    }

    let start = index.saturating_sub(config.runway_lookback);
    let mut nearest_barrier: Option<f64> = None;
    for candidate in (start + 2)..index.saturating_sub(1) {
        if candidate + 2 > index {
            break;
        }
        let high = dataset.frames_15m[candidate].candle.high;
        let left_one = dataset.frames_15m[candidate - 1].candle.high;
        let left_two = dataset.frames_15m[candidate - 2].candle.high;
        let right_one = dataset.frames_15m[candidate + 1].candle.high;
        let right_two = dataset.frames_15m[candidate + 2].candle.high;
        let is_local_high =
            high > left_one && high > left_two && high >= right_one && high >= right_two;
        if is_local_high && high > entry_price {
            nearest_barrier = match nearest_barrier {
                Some(existing) => Some(existing.min(high)),
                None => Some(high),
            };
        }
    }

    matches!(nearest_barrier, Some(barrier) if barrier - entry_price < config.stop_atr_multiple * atr)
}
