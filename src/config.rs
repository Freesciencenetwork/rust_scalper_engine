use serde::{Deserialize, Serialize};

use crate::domain::SymbolFilters;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub vol_baseline_lookback_bars: usize,
    pub high_vol_ratio: f64,
    pub daily_loss_limit_r: f64,
    pub risk_fraction: f64,
    pub min_target_move_pct: f64,
    pub tick_size: f64,
    pub lot_step: f64,
    pub ema_fast_period: usize,
    pub ema_slow_period: usize,
    pub atr_period: usize,
    pub vwma_lookback: usize,
    pub trend_confirm_bars: usize,
    pub breakout_lookback: usize,
    pub runway_lookback: usize,
    /// How far back (in bars) the failed-acceptance gate replays history to build
    /// its state. Keeping this short (default 96 = 24h) prevents a single old
    /// failed breakout from silencing the engine for the entire 1000-bar window.
    /// Must be > breakout_lookback to give the gate enough history to detect a breakout.
    pub failed_acceptance_lookback_bars: usize,
    pub stop_atr_multiple: f64,
    pub target_atr_multiple: f64,
    pub low_vol_enabled: bool,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            vol_baseline_lookback_bars: 960,
            high_vol_ratio: 1.8,
            daily_loss_limit_r: -2.0,
            risk_fraction: 0.005,
            min_target_move_pct: 0.0075,
            tick_size: 0.1,
            lot_step: 0.001,
            ema_fast_period: 9,
            ema_slow_period: 21,
            atr_period: 14,
            vwma_lookback: 96,
            trend_confirm_bars: 3,
            breakout_lookback: 20,
            runway_lookback: 40,
            failed_acceptance_lookback_bars: 96,
            stop_atr_multiple: 2.0,
            target_atr_multiple: 3.0,
            low_vol_enabled: true,
        }
    }
}

impl StrategyConfig {
    pub fn with_symbol_filters(mut self, filters: SymbolFilters) -> Self {
        self.tick_size = filters.tick_size;
        self.lot_step = filters.lot_step;
        self
    }
}
