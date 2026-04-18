use crate::config::StrategyConfig;
use crate::market_data::PreparedCandle;

/// Long continuation should not trigger from sustained trade **below** the value
/// area low (local auction not accepting higher prices).
pub fn favors_long_continuation(frame: &PreparedCandle, config: &StrategyConfig) -> bool {
    if !config.vp_enabled {
        return true;
    }
    match frame.vp_val {
        None => true,
        Some(val) => frame.candle.close >= val,
    }
}
