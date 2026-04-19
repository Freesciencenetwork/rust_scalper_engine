use crate::config::StrategyConfig;
use crate::market_data::PreparedCandle;
use crate::strategy::formulas::target_move_pct;

pub fn active(frame: &PreparedCandle, entry_price: f64, config: &StrategyConfig) -> bool {
    let Some(atr) = frame.atr else {
        return false;
    };
    target_move_pct(config.target_atr_multiple, atr, entry_price) < config.min_target_move_pct
}
