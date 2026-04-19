use crate::config::StrategyConfig;
use crate::market_data::PreparedDataset;

pub fn active(index: usize, dataset: &PreparedDataset, config: &StrategyConfig) -> bool {
    let Some(atr) = dataset.frames[index].atr else {
        return false;
    };
    let start = index.saturating_sub(1);
    for candidate in (start..=index).rev() {
        let candle = &dataset.frames[candidate].candle;
        if candle.close <= candle.open {
            continue;
        }
        if candidate + 1 < config.breakout_lookback {
            continue;
        }
        let window_start = candidate + 1 - config.breakout_lookback;
        let highest_high = dataset.frames[window_start..=candidate]
            .iter()
            .map(|frame| frame.candle.high)
            .fold(f64::MIN, f64::max);
        if candle.high < highest_high {
            continue;
        }
        let range = candle.high - candle.low;
        let body = candle.close - candle.open;
        let qualifies = range >= 2.5 * atr || body >= 1.75 * atr;
        if qualifies {
            let shock_mid = (candle.high + candle.low) / 2.0;
            return dataset.frames[index].candle.close < shock_mid;
        }
    }
    false
}
