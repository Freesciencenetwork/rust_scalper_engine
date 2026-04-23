//! Deterministic strategy backtest for trade-outcome labeling.
//!
//! V1 rules are intentionally conservative and simple:
//! - long-only
//! - one position at a time
//! - signal becomes a next-bar stop order
//! - order expires if the next bar does not trade through `trigger_price`
//! - stop wins on any same-bar stop/target conflict
//! - max-hold exits at the bar close

#![allow(clippy::pedantic, clippy::nursery)] // Small simulation engine; pedantic churn is high relative to value here.

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::machine::{EvaluateStrategyError, MachineRequest, resolve_replay_window_indices};
use crate::market_data::PreparedDataset;
use crate::strategies::{strategy_engine_for, supported_strategy_ids};
use crate::strategy::SignalDecision;
use crate::strategy::formulas::build_position_plan;

fn default_entry_fee_bps() -> f64 {
    10.0
}

fn default_exit_fee_bps() -> f64 {
    10.0
}

fn default_entry_slippage_bps() -> f64 {
    2.0
}

fn default_exit_slippage_bps() -> f64 {
    2.0
}

fn default_stop_extra_slippage_bps() -> f64 {
    3.0
}

fn default_max_hold_bars() -> usize {
    20
}

fn bps_to_ratio(bps: f64) -> f64 {
    bps / 10_000.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionAssumptions {
    #[serde(default = "default_entry_fee_bps")]
    pub entry_fee_bps: f64,
    #[serde(default = "default_exit_fee_bps")]
    pub exit_fee_bps: f64,
    #[serde(default = "default_entry_slippage_bps")]
    pub entry_slippage_bps: f64,
    #[serde(default = "default_exit_slippage_bps")]
    pub exit_slippage_bps: f64,
    #[serde(default = "default_stop_extra_slippage_bps")]
    pub stop_extra_slippage_bps: f64,
    #[serde(default = "default_max_hold_bars")]
    pub max_hold_bars: usize,
}

impl Default for ExecutionAssumptions {
    fn default() -> Self {
        Self {
            entry_fee_bps: default_entry_fee_bps(),
            exit_fee_bps: default_exit_fee_bps(),
            entry_slippage_bps: default_entry_slippage_bps(),
            exit_slippage_bps: default_exit_slippage_bps(),
            stop_extra_slippage_bps: default_stop_extra_slippage_bps(),
            max_hold_bars: default_max_hold_bars(),
        }
    }
}

impl ExecutionAssumptions {
    fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("entry_fee_bps", self.entry_fee_bps),
            ("exit_fee_bps", self.exit_fee_bps),
            ("entry_slippage_bps", self.entry_slippage_bps),
            ("exit_slippage_bps", self.exit_slippage_bps),
            ("stop_extra_slippage_bps", self.stop_extra_slippage_bps),
        ] {
            if !value.is_finite() {
                bail!("{name} must be finite");
            }
            if value < 0.0 {
                bail!("{name} must be >= 0");
            }
            if value >= 10_000.0 {
                bail!("{name} must be < 10000 bps");
            }
        }
        if self.max_hold_bars == 0 {
            bail!("max_hold_bars must be >= 1");
        }
        let stop_exit_bps = self.exit_slippage_bps + self.stop_extra_slippage_bps;
        if stop_exit_bps >= 10_000.0 {
            bail!("exit_slippage_bps + stop_extra_slippage_bps must be < 10000 bps");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyBacktestRequest {
    #[serde(flatten)]
    pub machine: MachineRequest,
    #[serde(default)]
    pub from_index: Option<usize>,
    #[serde(default)]
    pub to_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_to: Option<String>,
    #[serde(default)]
    pub execution: ExecutionAssumptions,
}

#[derive(Clone, Debug, Serialize)]
pub struct StrategyBacktestResponse {
    pub strategy_id: String,
    pub summary: BacktestSummary,
    pub trades: Vec<TradeOutcome>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TradeOutcome {
    pub signal_bar_index: usize,
    pub entry_bar_index: usize,
    pub exit_bar_index: usize,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub signal_close_time: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub entry_close_time: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub exit_close_time: DateTime<Utc>,
    pub entry_price_raw: f64,
    pub entry_price_fill: f64,
    pub exit_price_raw: f64,
    pub exit_price_fill: f64,
    pub trigger_price: f64,
    pub stop_price: f64,
    pub target_price: f64,
    pub atr_at_signal: f64,
    pub bars_held: usize,
    pub exit_reason: ExitReason,
    pub gross_return_pct: f64,
    pub gross_r: f64,
    pub fee_cost_pct: f64,
    pub slippage_cost_pct: f64,
    pub net_return_pct: f64,
    pub net_r: f64,
    pub profitable: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    TargetHit,
    StopHit,
    MaxHoldExpired,
}

#[derive(Clone, Debug, Serialize)]
pub struct BacktestSummary {
    pub trade_count: usize,
    pub win_count: usize,
    pub loss_count: usize,
    pub win_rate: f64,
    pub avg_gross_r: f64,
    pub avg_net_r: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profit_factor: Option<f64>,
    pub expectancy_r: f64,
    pub max_drawdown_r: f64,
    pub total_net_r: f64,
    pub avg_bars_held: f64,
}

#[derive(Clone, Copy, Debug)]
struct SignalSetup {
    trigger_price: f64,
    atr: f64,
}

#[derive(Clone, Copy, Debug)]
struct ExitFill {
    reason: ExitReason,
    exit_idx: usize,
    exit_price_raw: f64,
}

/// Simulate the configured strategy over the requested bar window.
pub fn simulate_backtest(
    config: &StrategyConfig,
    dataset: &PreparedDataset,
    from: usize,
    to: usize,
    exec: &ExecutionAssumptions,
    system_mode: SystemMode,
) -> Result<Vec<TradeOutcome>> {
    let mut engine = strategy_engine_for(config)?;
    engine.set_system_mode(system_mode);
    let mut failed_acceptance_cursor = 0usize;
    Ok(simulate_signal_stream(
        config,
        dataset,
        from,
        to,
        exec,
        |index| {
            if failed_acceptance_cursor <= index {
                engine.replay_failed_acceptance_window(failed_acceptance_cursor, index, dataset);
                failed_acceptance_cursor = index.saturating_add(1);
            }
            signal_setup_from_decision(engine.decide(index, dataset))
        },
    ))
}

pub fn compute_summary(trades: &[TradeOutcome]) -> BacktestSummary {
    if trades.is_empty() {
        return BacktestSummary {
            trade_count: 0,
            win_count: 0,
            loss_count: 0,
            win_rate: 0.0,
            avg_gross_r: 0.0,
            avg_net_r: 0.0,
            profit_factor: None,
            expectancy_r: 0.0,
            max_drawdown_r: 0.0,
            total_net_r: 0.0,
            avg_bars_held: 0.0,
        };
    }

    let trade_count = trades.len();
    let win_count = trades.iter().filter(|trade| trade.profitable).count();
    let loss_count = trade_count - win_count;
    let gross_sum: f64 = trades.iter().map(|trade| trade.gross_r).sum();
    let net_sum: f64 = trades.iter().map(|trade| trade.net_r).sum();
    let bars_sum: usize = trades.iter().map(|trade| trade.bars_held).sum();
    let positive_net_r: f64 = trades
        .iter()
        .filter(|trade| trade.net_r > 0.0)
        .map(|trade| trade.net_r)
        .sum();
    let negative_net_r: f64 = trades
        .iter()
        .filter(|trade| trade.net_r < 0.0)
        .map(|trade| trade.net_r)
        .sum();
    let profit_factor = if negative_net_r < 0.0 {
        Some(positive_net_r / negative_net_r.abs())
    } else {
        None
    };
    let max_drawdown_r = max_drawdown_r(trades);

    BacktestSummary {
        trade_count,
        win_count,
        loss_count,
        win_rate: win_count as f64 / trade_count as f64,
        avg_gross_r: gross_sum / trade_count as f64,
        avg_net_r: net_sum / trade_count as f64,
        profit_factor,
        expectancy_r: net_sum / trade_count as f64,
        max_drawdown_r,
        total_net_r: net_sum,
        avg_bars_held: bars_sum as f64 / trade_count as f64,
    }
}

impl crate::machine::DecisionMachine {
    /// Run a deterministic long-only backtest over the requested window and return a trade ledger.
    pub fn evaluate_backtest(
        &self,
        req: StrategyBacktestRequest,
    ) -> Result<StrategyBacktestResponse, EvaluateStrategyError> {
        req.execution
            .validate()
            .map_err(EvaluateStrategyError::Dataset)?;

        let halt = req.machine.runtime_state.halt_new_entries_flag != 0;
        let system_mode = if halt {
            SystemMode::Halted
        } else {
            SystemMode::Active
        };
        let (config, dataset) = self
            .prepare_dataset(req.machine)
            .map_err(EvaluateStrategyError::Dataset)?;
        if !supported_strategy_ids().contains(&config.strategy_id.as_str()) {
            return Err(EvaluateStrategyError::Unknown {
                id: config.strategy_id.clone(),
            });
        }
        let bar_count = dataset.frames.len();
        let last = bar_count.checked_sub(1).ok_or_else(|| {
            EvaluateStrategyError::Dataset(anyhow!("at least one closed candle is required"))
        })?;
        let (from_idx, to_idx) = resolve_replay_window_indices(
            &dataset.frames,
            last,
            req.from_index,
            req.to_index,
            req.replay_from.as_deref(),
            req.replay_to.as_deref(),
        )
        .map_err(EvaluateStrategyError::Dataset)?;
        if from_idx > to_idx {
            return Err(EvaluateStrategyError::Dataset(anyhow!(
                "from_index ({from_idx}) must be <= to_index ({to_idx}) after clamping to bar_count-1 ({last})"
            )));
        }
        if from_idx >= bar_count {
            return Err(EvaluateStrategyError::Dataset(anyhow!(
                "from_index ({from_idx}) must be < bar_count ({bar_count})"
            )));
        }

        let trades = simulate_backtest(
            &config,
            &dataset,
            from_idx,
            to_idx,
            &req.execution,
            system_mode,
        )
        .map_err(EvaluateStrategyError::Dataset)?;
        let summary = compute_summary(&trades);
        Ok(StrategyBacktestResponse {
            strategy_id: config.strategy_id,
            summary,
            trades,
        })
    }
}

fn signal_setup_from_decision(decision: SignalDecision) -> Option<SignalSetup> {
    if !decision.allowed {
        return None;
    }
    match (decision.trigger_price, decision.atr) {
        (Some(trigger_price), Some(atr)) if trigger_price > 0.0 && atr > 0.0 => {
            Some(SignalSetup { trigger_price, atr })
        }
        _ => None,
    }
}

fn simulate_signal_stream<F>(
    config: &StrategyConfig,
    dataset: &PreparedDataset,
    from: usize,
    to: usize,
    exec: &ExecutionAssumptions,
    mut signal_at: F,
) -> Vec<TradeOutcome>
where
    F: FnMut(usize) -> Option<SignalSetup>,
{
    let mut trades = Vec::new();
    let mut i = from;
    while i <= to {
        let Some(signal) = signal_at(i) else {
            i = i.saturating_add(1);
            continue;
        };
        let Some(entry_idx) = i.checked_add(1).filter(|&index| index <= to) else {
            break;
        };
        let entry_bar = &dataset.frames[entry_idx].candle;
        if entry_bar.high < signal.trigger_price {
            i = i.saturating_add(1);
            continue;
        }
        let Some(trade) = simulate_trade(config, dataset, i, entry_idx, to, signal, exec) else {
            break;
        };
        let next_index = trade.exit_bar_index.saturating_add(1);
        trades.push(trade);
        if next_index > to {
            break;
        }
        i = next_index;
    }
    trades
}

fn simulate_trade(
    config: &StrategyConfig,
    dataset: &PreparedDataset,
    signal_idx: usize,
    entry_idx: usize,
    to: usize,
    signal: SignalSetup,
    exec: &ExecutionAssumptions,
) -> Option<TradeOutcome> {
    let position = build_position_plan(config, signal.trigger_price, signal.atr, None);
    if position.risk_usd_per_btc <= 0.0 {
        return None;
    }
    let exit = find_exit(dataset, entry_idx, to, &position, exec)?;
    Some(build_trade_outcome(
        dataset, signal_idx, entry_idx, signal, &position, exec, exit,
    ))
}

fn find_exit(
    dataset: &PreparedDataset,
    entry_idx: usize,
    to: usize,
    position: &crate::strategy::formulas::PositionPlan,
    exec: &ExecutionAssumptions,
) -> Option<ExitFill> {
    let max_exit_idx = entry_idx
        .saturating_add(exec.max_hold_bars.saturating_sub(1))
        .min(to);
    for exit_idx in entry_idx..=max_exit_idx {
        let candle = &dataset.frames[exit_idx].candle;
        let stop_hit = candle.low <= position.stop_price;
        let target_hit = candle.high >= position.target_price;
        if stop_hit {
            return Some(ExitFill {
                reason: ExitReason::StopHit,
                exit_idx,
                exit_price_raw: position.stop_price,
            });
        }
        if target_hit {
            return Some(ExitFill {
                reason: ExitReason::TargetHit,
                exit_idx,
                exit_price_raw: position.target_price,
            });
        }
        let bars_held = exit_idx - entry_idx + 1;
        if bars_held >= exec.max_hold_bars {
            return Some(ExitFill {
                reason: ExitReason::MaxHoldExpired,
                exit_idx,
                exit_price_raw: candle.close,
            });
        }
    }
    None
}

fn build_trade_outcome(
    dataset: &PreparedDataset,
    signal_idx: usize,
    entry_idx: usize,
    signal: SignalSetup,
    position: &crate::strategy::formulas::PositionPlan,
    exec: &ExecutionAssumptions,
    exit: ExitFill,
) -> TradeOutcome {
    let entry_price_raw = signal.trigger_price;
    let entry_price_fill = entry_price_raw * (1.0 + bps_to_ratio(exec.entry_slippage_bps));
    let exit_slippage_bps = exec.exit_slippage_bps
        + if exit.reason == ExitReason::StopHit {
            exec.stop_extra_slippage_bps
        } else {
            0.0
        };
    let exit_price_fill = exit.exit_price_raw * (1.0 - bps_to_ratio(exit_slippage_bps));
    let entry_fee_abs = entry_price_fill * bps_to_ratio(exec.entry_fee_bps);
    let exit_fee_abs = exit_price_fill * bps_to_ratio(exec.exit_fee_bps);
    let fee_cost_abs = entry_fee_abs + exit_fee_abs;
    let entry_slippage_abs = entry_price_fill - entry_price_raw;
    let exit_slippage_abs = exit.exit_price_raw - exit_price_fill;
    let slippage_cost_abs = entry_slippage_abs + exit_slippage_abs;
    let gross_pnl = exit.exit_price_raw - entry_price_raw;
    let net_pnl = exit_price_fill - entry_price_fill - fee_cost_abs;
    let risk_per_unit = position.risk_usd_per_btc;
    let bars_held = exit.exit_idx - entry_idx + 1;

    TradeOutcome {
        signal_bar_index: signal_idx,
        entry_bar_index: entry_idx,
        exit_bar_index: exit.exit_idx,
        signal_close_time: dataset.frames[signal_idx].candle.close_time,
        entry_close_time: dataset.frames[entry_idx].candle.close_time,
        exit_close_time: dataset.frames[exit.exit_idx].candle.close_time,
        entry_price_raw,
        entry_price_fill,
        exit_price_raw: exit.exit_price_raw,
        exit_price_fill,
        trigger_price: signal.trigger_price,
        stop_price: position.stop_price,
        target_price: position.target_price,
        atr_at_signal: signal.atr,
        bars_held,
        exit_reason: exit.reason,
        gross_return_pct: gross_pnl / entry_price_raw,
        gross_r: gross_pnl / risk_per_unit,
        fee_cost_pct: fee_cost_abs / entry_price_raw,
        slippage_cost_pct: slippage_cost_abs / entry_price_raw,
        net_return_pct: net_pnl / entry_price_raw,
        net_r: net_pnl / risk_per_unit,
        profitable: net_pnl > 0.0,
    }
}

fn max_drawdown_r(trades: &[TradeOutcome]) -> f64 {
    let mut peak = 0.0;
    let mut equity = 0.0;
    let mut max_drawdown = 0.0;
    for trade in trades {
        equity += trade.net_r;
        if equity > peak {
            peak = equity;
        }
        let drawdown = peak - equity;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }
    max_drawdown
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::{
        ExecutionAssumptions, ExitReason, SignalSetup, TradeOutcome, compute_summary,
        simulate_signal_stream,
    };
    use crate::config::StrategyConfig;
    use crate::domain::Candle;
    use crate::market_data::{IndicatorSnapshot, PreparedCandle, PreparedDataset};

    fn candle(base_idx: i64, open: f64, high: f64, low: f64, close: f64) -> Candle {
        Candle {
            close_time: Utc
                .with_ymd_and_hms(2026, 4, 22, 0, 0, 0)
                .single()
                .expect("base time")
                + Duration::minutes(base_idx),
            open,
            high,
            low,
            close,
            volume: 1.0,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        }
    }

    fn dataset(candles: Vec<Candle>) -> PreparedDataset {
        PreparedDataset {
            frames: candles
                .into_iter()
                .map(|candle| PreparedCandle {
                    candle,
                    ema_fast: None,
                    ema_slow: None,
                    ema_fast_higher: None,
                    ema_slow_higher: None,
                    vwma: None,
                    atr: None,
                    atr_pct: None,
                    atr_pct_baseline: None,
                    vol_ratio: None,
                    cvd_ema3: None,
                    cvd_ema3_slope: None,
                    vp_val: None,
                    vp_poc: None,
                    vp_vah: None,
                    indicator_snapshot: IndicatorSnapshot::default(),
                })
                .collect(),
            macro_events: Vec::new(),
        }
    }

    fn config() -> StrategyConfig {
        StrategyConfig {
            tick_size: 0.1,
            stop_atr_multiple: 1.0,
            target_atr_multiple: 2.0,
            ..Default::default()
        }
    }

    fn signal_map(indices: &[(usize, SignalSetup)]) -> Vec<Option<SignalSetup>> {
        let max_index = indices.iter().map(|(index, _)| *index).max().unwrap_or(0);
        let mut out = vec![None; max_index + 1];
        for &(index, setup) in indices {
            out[index] = Some(setup);
        }
        out
    }

    fn run_simulation(
        candles: Vec<Candle>,
        signals: &[(usize, SignalSetup)],
        exec: ExecutionAssumptions,
    ) -> Vec<TradeOutcome> {
        let dataset = dataset(candles);
        let signal_plan = signal_map(signals);
        simulate_signal_stream(
            &config(),
            &dataset,
            0,
            dataset.frames.len().saturating_sub(1),
            &exec,
            |index| signal_plan.get(index).copied().flatten(),
        )
    }

    #[test]
    fn test_entry_fill_on_next_bar() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 100.2, 99.0, 100.0),
                candle(2, 100.0, 100.4, 99.8, 100.3),
            ],
            &[(
                0,
                SignalSetup {
                    trigger_price: 100.0,
                    atr: 2.0,
                },
            )],
            ExecutionAssumptions {
                max_hold_bars: 2,
                ..Default::default()
            },
        );
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].entry_bar_index, 1);
    }

    #[test]
    fn test_signal_expires_if_no_trigger() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 99.9, 99.0, 99.5),
                candle(2, 99.5, 100.4, 99.4, 100.2),
            ],
            &[(
                0,
                SignalSetup {
                    trigger_price: 100.0,
                    atr: 2.0,
                },
            )],
            ExecutionAssumptions::default(),
        );
        assert!(trades.is_empty());
    }

    #[test]
    fn test_stop_hit() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 100.2, 99.1, 100.0),
                candle(2, 100.0, 100.3, 97.8, 98.2),
            ],
            &[(
                0,
                SignalSetup {
                    trigger_price: 100.0,
                    atr: 2.0,
                },
            )],
            ExecutionAssumptions::default(),
        );
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::StopHit);
        assert_eq!(trades[0].exit_price_raw, 98.0);
        assert!(trades[0].exit_price_fill < 98.0);
    }

    #[test]
    fn test_target_hit() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 100.2, 99.1, 100.0),
                candle(2, 100.0, 104.2, 99.8, 103.8),
            ],
            &[(
                0,
                SignalSetup {
                    trigger_price: 100.0,
                    atr: 2.0,
                },
            )],
            ExecutionAssumptions::default(),
        );
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::TargetHit);
        assert_eq!(trades[0].exit_price_raw, 104.0);
        assert!(trades[0].exit_price_fill < 104.0);
    }

    #[test]
    fn test_same_bar_stop_wins() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 104.2, 97.8, 100.0),
            ],
            &[(
                0,
                SignalSetup {
                    trigger_price: 100.0,
                    atr: 2.0,
                },
            )],
            ExecutionAssumptions::default(),
        );
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].entry_bar_index, 1);
        assert_eq!(trades[0].exit_bar_index, 1);
        assert_eq!(trades[0].exit_reason, ExitReason::StopHit);
    }

    #[test]
    fn test_max_hold_exit() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 100.2, 99.1, 100.0),
                candle(2, 100.0, 100.3, 99.5, 100.2),
                candle(3, 100.2, 100.3, 99.6, 100.4),
            ],
            &[(
                0,
                SignalSetup {
                    trigger_price: 100.0,
                    atr: 2.0,
                },
            )],
            ExecutionAssumptions {
                max_hold_bars: 3,
                ..Default::default()
            },
        );
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].exit_reason, ExitReason::MaxHoldExpired);
        assert_eq!(trades[0].exit_price_raw, 100.4);
        assert_eq!(trades[0].bars_held, 3);
    }

    #[test]
    fn test_fee_and_slippage_arithmetic() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 100.2, 99.1, 100.0),
                candle(2, 100.0, 104.2, 99.8, 103.8),
            ],
            &[(
                0,
                SignalSetup {
                    trigger_price: 100.0,
                    atr: 2.0,
                },
            )],
            ExecutionAssumptions::default(),
        );
        let trade = &trades[0];
        let combined_cost_pct = trade.fee_cost_pct + trade.slippage_cost_pct;
        let residual = trade.gross_return_pct - combined_cost_pct - trade.net_return_pct;
        assert!(residual.abs() < 1e-9, "residual={residual}");
        assert!(trade.net_r < trade.gross_r);
    }

    #[test]
    fn test_one_position_at_a_time() {
        let trades = run_simulation(
            vec![
                candle(0, 99.0, 99.5, 98.5, 99.2),
                candle(1, 99.2, 100.2, 99.1, 100.0),
                candle(2, 100.0, 100.3, 99.5, 100.2),
                candle(3, 100.2, 100.3, 99.6, 100.4),
                candle(4, 100.4, 100.5, 99.8, 100.6),
            ],
            &[
                (
                    0,
                    SignalSetup {
                        trigger_price: 100.0,
                        atr: 2.0,
                    },
                ),
                (
                    1,
                    SignalSetup {
                        trigger_price: 100.1,
                        atr: 2.0,
                    },
                ),
            ],
            ExecutionAssumptions {
                max_hold_bars: 3,
                ..Default::default()
            },
        );
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].signal_bar_index, 0);
    }

    #[test]
    fn test_summary_profit_factor() {
        let summary = compute_summary(&[
            trade_with_net_r(2.0),
            trade_with_net_r(2.0),
            trade_with_net_r(-1.0),
        ]);
        assert_eq!(summary.trade_count, 3);
        assert_eq!(summary.profit_factor, Some(4.0));
    }

    #[test]
    fn test_summary_max_drawdown() {
        let summary = compute_summary(&[
            trade_with_net_r(1.0),
            trade_with_net_r(-0.5),
            trade_with_net_r(-1.0),
            trade_with_net_r(0.25),
        ]);
        assert!((summary.max_drawdown_r - 1.5).abs() < 1e-9);
    }

    fn trade_with_net_r(net_r: f64) -> TradeOutcome {
        let base_time = Utc
            .with_ymd_and_hms(2026, 4, 22, 0, 0, 0)
            .single()
            .expect("base time");
        TradeOutcome {
            signal_bar_index: 0,
            entry_bar_index: 1,
            exit_bar_index: 2,
            signal_close_time: base_time,
            entry_close_time: base_time,
            exit_close_time: base_time,
            entry_price_raw: 100.0,
            entry_price_fill: 100.0,
            exit_price_raw: 102.0,
            exit_price_fill: 102.0,
            trigger_price: 100.0,
            stop_price: 99.0,
            target_price: 102.0,
            atr_at_signal: 1.0,
            bars_held: 2,
            exit_reason: ExitReason::TargetHit,
            gross_return_pct: 0.02,
            gross_r: net_r,
            fee_cost_pct: 0.0,
            slippage_cost_pct: 0.0,
            net_return_pct: net_r / 100.0,
            net_r,
            profitable: net_r > 0.0,
        }
    }
}
