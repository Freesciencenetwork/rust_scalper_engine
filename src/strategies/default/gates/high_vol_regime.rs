use crate::config::StrategyConfig;
use crate::domain::VolatilityRegime;
use crate::market_data::PreparedCandle;

pub fn regime(frame: &PreparedCandle, config: &StrategyConfig) -> VolatilityRegime {
    match frame.vol_ratio {
        Some(vol_ratio) if vol_ratio >= config.high_vol_ratio => VolatilityRegime::High,
        _ => VolatilityRegime::Normal,
    }
}
