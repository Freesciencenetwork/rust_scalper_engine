use crate::config::StrategyConfig;

pub fn passes(index: usize, config: &StrategyConfig) -> bool {
    let mut min_bars = config
        .vol_baseline_lookback_bars
        .max(config.vwma_lookback)
        .max(config.runway_lookback);
    if config.vp_enabled {
        min_bars = min_bars.max(config.vp_lookback_bars);
    }
    index + 1 >= min_bars
}
