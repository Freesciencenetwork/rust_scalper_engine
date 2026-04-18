use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::StrategyConfig;
use crate::context::overlay::ParameterOverlay;
use crate::context::policy::apply_overlay_to_config;
use crate::context::rustyfish::{RustyFishDailyReport, map_report_to_overlay};
use crate::domain::{Candle, MacroEvent, SymbolFilters, SystemMode};
use crate::strategy::formulas::{PositionPlan, build_position_plan};
use crate::strategy::{PreparedCandle, PreparedDataset, SignalDecision, StrategyEngine};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default)]
    pub realized_net_r_today: f64,
    #[serde(default)]
    pub halt_new_entries_flag: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineRequest {
    pub candles_15m: Vec<Candle>,
    #[serde(default)]
    pub macro_events: Vec<MacroEvent>,
    #[serde(default)]
    pub runtime_state: RuntimeState,
    pub account_equity: Option<f64>,
    pub symbol_filters: Option<SymbolFilters>,
    pub rustyfish_overlay: Option<RustyFishDailyReport>,
    #[serde(default)]
    pub config_overrides: Option<ConfigOverrides>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigOverrides {
    pub min_target_move_pct: Option<f64>,
    pub stop_atr_multiple: Option<f64>,
    pub target_atr_multiple: Option<f64>,
    pub runway_lookback: Option<usize>,
    pub ema_fast_period: Option<usize>,
    pub ema_slow_period: Option<usize>,
    pub low_vol_enabled: Option<bool>,
    pub high_vol_ratio: Option<f64>,
    pub breakout_lookback: Option<usize>,
    pub failed_acceptance_lookback_bars: Option<usize>,
    pub trend_confirm_bars: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineAction {
    StandAside,
    ArmLongStop,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineDiagnostics {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub as_of: DateTime<Utc>,
    pub latest_frame: PreparedCandle,
    pub effective_config: StrategyConfig,
    pub overlay: Option<ParameterOverlay>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineCapabilities {
    pub machine_name: String,
    pub machine_version: String,
    pub execution_enabled: bool,
    pub supported_actions: Vec<String>,
    pub accepted_inputs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineResponse {
    pub action: MachineAction,
    pub decision: SignalDecision,
    pub plan: Option<PositionPlan>,
    pub diagnostics: MachineDiagnostics,
}

#[derive(Clone, Debug)]
pub struct DecisionMachine {
    base_config: StrategyConfig,
}

impl Default for DecisionMachine {
    fn default() -> Self {
        Self::new(StrategyConfig::default())
    }
}

impl DecisionMachine {
    pub fn new(base_config: StrategyConfig) -> Self {
        Self { base_config }
    }

    pub fn capabilities(&self) -> MachineCapabilities {
        MachineCapabilities {
            machine_name: "binance_BTC_machine".to_string(),
            machine_version: env!("CARGO_PKG_VERSION").to_string(),
            execution_enabled: false,
            supported_actions: vec!["stand_aside".to_string(), "arm_long_stop".to_string()],
            accepted_inputs: vec![
                "normalized_15m_candles".to_string(),
                "macro_events_numeric".to_string(),
                "symbol_filters".to_string(),
                "runtime_state_numeric".to_string(),
                "rustyfish_overlay_numeric".to_string(),
            ],
        }
    }

    pub fn evaluate(&self, request: MachineRequest) -> Result<MachineResponse> {
        let mut config = self.base_config.clone();

        if let Some(filters) = request.symbol_filters.clone() {
            config = config.with_symbol_filters(filters);
        }

        if let Some(ov) = &request.config_overrides {
            if let Some(v) = ov.min_target_move_pct { config.min_target_move_pct = v; }
            if let Some(v) = ov.stop_atr_multiple { config.stop_atr_multiple = v; }
            if let Some(v) = ov.target_atr_multiple { config.target_atr_multiple = v; }
            if let Some(v) = ov.runway_lookback { config.runway_lookback = v; }
            if let Some(v) = ov.ema_fast_period { config.ema_fast_period = v; }
            if let Some(v) = ov.ema_slow_period { config.ema_slow_period = v; }
            if let Some(v) = ov.low_vol_enabled { config.low_vol_enabled = v; }
            if let Some(v) = ov.high_vol_ratio { config.high_vol_ratio = v; }
            if let Some(v) = ov.breakout_lookback { config.breakout_lookback = v; }
            if let Some(v) = ov.failed_acceptance_lookback_bars { config.failed_acceptance_lookback_bars = v; }
            if let Some(v) = ov.trend_confirm_bars { config.trend_confirm_bars = v; }
        }

        let overlay = request
            .rustyfish_overlay
            .as_ref()
            .map(map_report_to_overlay);
        if let Some(overlay) = overlay.as_ref() {
            config = apply_overlay_to_config(&config, overlay);
        }

        let dataset = PreparedDataset::build(&config, request.candles_15m, request.macro_events)?;
        let index = dataset
            .frames_15m
            .len()
            .checked_sub(1)
            .ok_or_else(|| anyhow::anyhow!("at least one closed 15m candle is required"))?;

        let mut engine = StrategyEngine::new(config.clone());
        if request.runtime_state.halt_new_entries_flag != 0
            || request.runtime_state.realized_net_r_today <= config.daily_loss_limit_r
        {
            engine.system_mode = SystemMode::Halted;
        }

        // Only replay the recent window for failed-acceptance state. Replaying
        // the full 1000-bar history causes a single old failed breakout to latch
        // the gate for the remainder of the window. Using a short lookback keeps
        // the gate responsive to *recent* price structure only.
        let fa_start = index.saturating_sub(config.failed_acceptance_lookback_bars);
        for frame_index in fa_start..=index {
            engine.update_failed_acceptance(frame_index, &dataset);
        }

        let decision = engine.evaluate_signal(index, &dataset);
        let plan = build_plan(&config, &decision, request.account_equity);
        let action = if decision.allowed {
            MachineAction::ArmLongStop
        } else {
            MachineAction::StandAside
        };
        let latest_frame = dataset.frames_15m[index].clone();

        Ok(MachineResponse {
            action,
            decision,
            plan,
            diagnostics: MachineDiagnostics {
                as_of: latest_frame.candle.close_time,
                latest_frame,
                effective_config: config,
                overlay,
            },
        })
    }
}

fn build_plan(
    config: &StrategyConfig,
    decision: &SignalDecision,
    account_equity: Option<f64>,
) -> Option<PositionPlan> {
    if !decision.allowed {
        return None;
    }

    let trigger_price = decision.trigger_price?;
    let atr = decision.atr?;
    if atr <= 0.0 {
        return None;
    }

    Some(build_position_plan(
        config,
        trigger_price,
        atr,
        account_equity,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::{DecisionMachine, MachineAction, MachineRequest, RuntimeState};
    use crate::domain::{Candle, MacroEvent, MacroEventClass};

    #[test]
    fn capabilities_are_explicitly_execution_free() {
        let machine = DecisionMachine::default();
        let capabilities = machine.capabilities();
        assert!(!capabilities.execution_enabled);
        assert!(
            capabilities
                .supported_actions
                .iter()
                .any(|value| value == "arm_long_stop")
        );
    }

    #[test]
    fn insufficient_history_blocks_entries() {
        let machine = DecisionMachine::default();
        let base_time = Utc
            .with_ymd_and_hms(2026, 4, 15, 0, 15, 0)
            .single()
            .expect("time");
        let request = MachineRequest {
            candles_15m: vec![Candle {
                close_time: base_time,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 10.0,
                buy_volume: Some(6.0),
                sell_volume: Some(4.0),
                delta: None,
            }],
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: Some(100_000.0),
            symbol_filters: None,
            rustyfish_overlay: None,
            config_overrides: None,
        };

        let result = machine.evaluate(request);
        assert!(result.is_err());

        let long_enough_request = MachineRequest {
            candles_15m: (0..96)
                .map(|index| Candle {
                    close_time: base_time + Duration::minutes(15 * index as i64),
                    open: 100.0 + index as f64 * 0.1,
                    high: 101.0 + index as f64 * 0.1,
                    low: 99.5 + index as f64 * 0.1,
                    close: 100.7 + index as f64 * 0.1,
                    volume: 10.0 + index as f64,
                    buy_volume: Some(6.0 + index as f64 * 0.1),
                    sell_volume: Some(4.0 + index as f64 * 0.1),
                    delta: None,
                })
                .collect(),
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: Some(100_000.0),
            symbol_filters: None,
            rustyfish_overlay: None,
            config_overrides: None,
        };

        let response = machine
            .evaluate(long_enough_request)
            .expect("machine response");
        assert!(matches!(
            response.action,
            MachineAction::StandAside | MachineAction::ArmLongStop
        ));
    }

    #[test]
    fn simulated_request_hits_macro_veto_and_returns_decision_package() {
        let machine = DecisionMachine::default();
        let base_time = Utc
            .with_ymd_and_hms(2026, 4, 15, 0, 15, 0)
            .single()
            .expect("time");

        let candles_15m: Vec<Candle> = (0..970)
            .map(|index| {
                let trend = index as f64 * 0.8;
                let close = 80_000.0 + trend + if index == 969 { 3.0 } else { 0.0 };
                let open = close - if index == 969 { 2.0 } else { 1.0 };
                let high = close + 4.0;
                let low = if index == 969 {
                    close - 6.0
                } else {
                    open - 2.0
                };
                Candle {
                    close_time: base_time + Duration::minutes(15 * index as i64),
                    open,
                    high,
                    low,
                    close,
                    volume: 100.0 + index as f64 * 0.2,
                    buy_volume: Some(60.0 + index as f64 * 0.1),
                    sell_volume: Some(40.0 + index as f64 * 0.05),
                    delta: None,
                }
            })
            .collect();

        let latest_close_time = candles_15m.last().expect("latest candle").close_time;

        let request = MachineRequest {
            candles_15m,
            macro_events: vec![MacroEvent {
                event_time: latest_close_time + Duration::minutes(10),
                class: MacroEventClass::Cpi,
            }],
            runtime_state: RuntimeState::default(),
            account_equity: Some(100_000.0),
            symbol_filters: None,
            rustyfish_overlay: None,
            config_overrides: None,
        };

        let request_json = serde_json::to_string(&request).expect("serialize request");
        let parsed_request: MachineRequest =
            serde_json::from_str(&request_json).expect("deserialize request");

        let response = machine.evaluate(parsed_request).expect("machine response");
        assert!(matches!(response.action, MachineAction::StandAside));
        assert!(!response.decision.allowed);
        assert!(
            response
                .decision
                .reasons
                .iter()
                .any(|reason| reason == "macro_veto")
        );
        assert!(response.plan.is_none());
        assert_eq!(response.diagnostics.as_of, latest_close_time);

        let response_json = serde_json::to_value(&response).expect("serialize response");
        assert_eq!(response_json["action"], "stand_aside");
        assert_eq!(
            response_json["diagnostics"]["as_of"].as_i64(),
            Some(latest_close_time.timestamp_millis())
        );
    }
}
