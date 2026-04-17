use crate::config::StrategyConfig;

pub fn passes(index: usize, config: &StrategyConfig) -> bool {
    index + 1
        >= config
            .vol_baseline_lookback_bars
            .max(config.vwma_lookback)
            .max(config.runway_lookback)
}
