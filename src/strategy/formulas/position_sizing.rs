use serde::{Deserialize, Serialize};

use crate::config::StrategyConfig;

use super::price_rounding::{floor_to_step, round_down_to_step, round_up_to_step};
use super::target_move_pct;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionPlan {
    pub trigger_price: f64,
    pub stop_price: f64,
    pub target_price: f64,
    pub target_move_pct: f64,
    pub risk_fraction: f64,
    pub risk_budget_usd: Option<f64>,
    pub risk_usd_per_btc: f64,
    pub qty_btc: Option<f64>,
}

pub fn build_position_plan(
    config: &StrategyConfig,
    trigger_price: f64,
    atr: f64,
    equity: Option<f64>,
) -> PositionPlan {
    let raw_stop = trigger_price - config.stop_atr_multiple * atr;
    let stop_price = round_down_to_step(raw_stop, config.tick_size);
    let raw_target = trigger_price + config.target_atr_multiple * atr;
    let target_price = round_up_to_step(raw_target, config.tick_size);
    let risk_usd_per_btc = (trigger_price - stop_price).max(0.0);
    let risk_budget_usd = equity.map(|value| value * config.risk_fraction);
    let qty_btc = risk_budget_usd.map(|budget| {
        let qty_raw = if risk_usd_per_btc > 0.0 {
            budget / risk_usd_per_btc
        } else {
            0.0
        };
        floor_to_step(qty_raw, config.lot_step)
    });

    PositionPlan {
        trigger_price,
        stop_price,
        target_price,
        risk_budget_usd,
        risk_usd_per_btc,
        qty_btc,
        target_move_pct: target_move_pct(config.target_atr_multiple, atr, trigger_price),
        risk_fraction: config.risk_fraction,
    }
}
