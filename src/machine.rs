#![allow(clippy::pedantic, clippy::nursery)] // Request orchestration; pedantic on large evaluate() is low signal.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::config::StrategyConfig;
use crate::domain::{Candle, MacroEvent, SymbolFilters, SystemMode};
use crate::historical_data::{BundledBtcUsd1m, load_btcusd_1m};
use crate::market_data::{PreparedCandle, PreparedDataset};
use crate::strategies::{strategy_engine_for, supported_strategy_ids};
use crate::strategy::decision::SignalDecision;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default)]
    pub realized_net_r_today: f64,
    #[serde(default)]
    pub halt_new_entries_flag: u8,
}

/// Server-generated **uniform** OHLCV for demos and smoke tests. Mutually exclusive with a non-empty
/// **`candles`** array: send either real `candles` **or** this block.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyntheticSeries {
    /// Milliseconds between consecutive bar `close_time` values. Omit when top-level **`bar_interval`**
    /// is set to a parseable label (e.g. `"15m"`, `"1h"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar_step_ms: Option<u64>,
    /// First bar close time (UTC epoch **milliseconds**). Defaults to a fixed anchor when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_close_ms: Option<i64>,
    /// Last bar close time (**inclusive**). Ignored when **`bar_count`** is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_close_ms: Option<i64>,
    /// Exact number of bars from **`start_close_ms`**. When **`end_close_ms`** and **`bar_count`** are both
    /// absent, defaults to **512** bars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar_count: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineRequest {
    /// Closed OHLCV bars, **oldest first**. JSON may use **`candles_15m`** as a backward-compat alias (legacy name from `binance-fetch`; it does **not** imply the bars are 15m — use **`bar_interval`** for timeframe).
    /// Leave empty when using [`SyntheticSeries`] or [`crate::historical_data::BundledBtcUsd1m`] instead.
    #[serde(default, alias = "candles_15m")]
    pub candles: Vec<Candle>,
    /// Optional label for what each row represents (e.g. `"15m"`, `"1h"`, `"4h"`). The engine still
    /// treats the series as **uniform steps**; warmup hints are expressed in **row counts**, not minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar_interval: Option<String>,
    #[serde(default)]
    pub macro_events: Vec<MacroEvent>,
    #[serde(default)]
    pub runtime_state: RuntimeState,
    pub account_equity: Option<f64>,
    pub symbol_filters: Option<SymbolFilters>,
    #[serde(default)]
    pub config_overrides: Option<ConfigOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthetic_series: Option<SyntheticSeries>,
    /// Bundled **`btcusd_1-min_data.csv`** slice (UTC calendar **`from`**/**`to`** or **`all: true`**).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundled_btcusd_1m: Option<BundledBtcUsd1m>,
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
    pub vp_enabled: Option<bool>,
    pub vp_lookback_bars: Option<usize>,
    pub vp_value_area_ratio: Option<f64>,
    pub vp_bin_count: Option<usize>,
    pub strategy_id: Option<String>,
    /// VWAP anchor mode. `"utc_day"` resets at UTC midnight — fine for sub-daily bars.
    /// For daily/weekly bars use `"rolling_bars"` or `"disabled"`.
    pub vwap_anchor_mode: Option<crate::config::VwapAnchorMode>,
    /// Rolling-window bar count for `vwap_anchor_mode = "rolling_bars"`.
    pub vwap_rolling_bars: Option<usize>,
    /// Base bars that roll into one higher-TF bar for `ema_fast_higher` / `ema_slow_higher`.
    /// `4` = every 4 base bars (default, matches 15m→1h). Set `1` to disable.
    pub higher_tf_factor: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineCapabilities {
    pub machine_name: String,
    pub machine_version: String,
    pub execution_enabled: bool,
    pub supported_actions: Vec<String>,
    pub accepted_inputs: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IndicatorValueReport {
    pub value: JsonValue,
    /// `true` when the value is non-null **and** (if known) `bars_available >= min_bars_required`.
    pub computable: bool,
    /// Minimum closed bars in **this** series before the path is typically defined (`None` = not catalogued).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_bars_required: Option<u32>,
    /// Number of closed bars you sent (`candles` / `candles_15m` length).
    pub bars_available: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_note: Option<&'static str>,
}

/// Last-bar snapshot for **one** catalog dot-path (same strings as `GET /v1/catalog` → `indicators[].path`).
#[derive(Clone, Debug, Serialize)]
pub struct IndicatorEvaluateResponse {
    pub path: String,
    #[serde(flatten)]
    pub report: IndicatorValueReport,
}

/// [`DecisionMachine::evaluate_indicator`] failures (HTTP layer maps [`EvaluateIndicatorError::Unknown`] to 404).
#[derive(Debug)]
pub enum EvaluateIndicatorError {
    Unknown { path: String },
    Dataset(anyhow::Error),
}

impl std::fmt::Display for EvaluateIndicatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown { path } => write!(f, "unknown_indicator: {path}"),
            Self::Dataset(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EvaluateIndicatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dataset(e) => Some(e.as_ref()),
            Self::Unknown { .. } => None,
        }
    }
}

/// Request body for indicator replay endpoints.
///
/// Serde-flattens [`MachineRequest`] into the JSON root (`candles`, optional `bar_interval`, …)
/// alongside `from_index` / `to_index` / `step`. Multi replay (`POST /v1/indicators/replay`) also
/// requires non-empty `indicators`; the single-indicator URL variant ignores `indicators` (path
/// comes from the URL).
///
/// **Calendar window:** set both **`replay_from`** and **`replay_to`** as UTC **`YYYY-MM-DD`**
/// strings. The server picks every bar whose **`close_time`** lies in that inclusive UTC day range
/// (midnight `replay_from` through end of `replay_to`). When both are set, they **override**
/// **`from_index`** / **`to_index`**. Otherwise use indices (defaults: `0` … last bar).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndicatorReplayRequest {
    #[serde(flatten)]
    pub machine: MachineRequest,
    /// Inclusive start bar index (`0`..`len-1`). Default `0`. Ignored when **`replay_from`** and
    /// **`replay_to`** are both set.
    #[serde(default)]
    pub from_index: Option<usize>,
    /// Inclusive end bar index. Default: last closed bar (`len-1`). Ignored when both replay day
    /// fields are set.
    #[serde(default)]
    pub to_index: Option<usize>,
    /// Emit every Nth bar in the range (`>= 1`). Default `1`.
    #[serde(default)]
    pub step: Option<usize>,
    /// Inclusive UTC calendar **start** day for replay, by each bar's **`close_time`** (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_from: Option<String>,
    /// Inclusive UTC calendar **end** day for replay (`YYYY-MM-DD`). Must be set together with
    /// **`replay_from`**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_to: Option<String>,
    /// Dot-path strings from `GET /v1/catalog` → `indicators[].path`. Required (non-empty) for
    /// `POST /v1/indicators/replay`; ignored for the single-indicator URL variant.
    #[serde(default)]
    pub indicators: Vec<String>,
}

/// One bar's worth of indicator values within an [`IndicatorReplayResponse`].
#[derive(Clone, Debug, Serialize)]
pub struct IndicatorReplayStep {
    pub bar_index: usize,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub close_time: DateTime<Utc>,
    /// Keyed by the same dot-path strings as `GET /v1/catalog` → `indicators[].path`.
    pub indicators: BTreeMap<String, IndicatorValueReport>,
    /// Paths requested but not found in the catalog (unknown dot-paths).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unknown_paths: Vec<String>,
}

/// Response for both single- and multi-indicator replay endpoints.
#[derive(Clone, Debug, Serialize)]
pub struct IndicatorReplayResponse {
    pub steps: Vec<IndicatorReplayStep>,
}

/// Linear **strategy** replay: same candle payload as [`MachineRequest`], plus optional bar window.
///
/// **“Timeframe” from the caller’s perspective:** each `candles` row is one bar at the TF you
/// fetched (15m, 1h, …). [`MachineRequest::bar_interval`] is only a label; replay uses **indices**
/// into that array, not a separate timeframe selector. Defaults: `from_index = 0`, `to_index =
/// last bar`, `step = 1` — walk the whole POSTed series linearly, capped at [`MAX_REPLAY_STEPS`].
///
/// **`replay_from`** / **`replay_to`** (`YYYY-MM-DD` UTC) work like [`IndicatorReplayRequest`]: when
/// both are set they override **`from_index`** / **`to_index`**.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyReplayRequest {
    #[serde(flatten)]
    pub machine: MachineRequest,
    #[serde(default)]
    pub from_index: Option<usize>,
    #[serde(default)]
    pub to_index: Option<usize>,
    #[serde(default)]
    pub step: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_to: Option<String>,
}

/// One bar’s [`SignalDecision`] after replaying failed-acceptance state through that index.
#[derive(Clone, Debug, Serialize)]
pub struct StrategyReplayStep {
    pub bar_index: usize,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub close_time: DateTime<Utc>,
    pub decision: SignalDecision,
}

#[derive(Clone, Debug, Serialize)]
pub struct StrategyReplayResponse {
    pub strategy_id: String,
    pub steps: Vec<StrategyReplayStep>,
}

/// Last-bar [`SignalDecision`] for the configured strategy (same **`strategy_id`** strings as
/// **`GET /v1/catalog`** → **`strategies[].id`**).
#[derive(Clone, Debug, Serialize)]
pub struct StrategyEvaluateResponse {
    pub strategy_id: String,
    pub decision: SignalDecision,
}

#[derive(Debug)]
pub enum EvaluateStrategyError {
    Unknown { id: String },
    Dataset(anyhow::Error),
}

impl std::fmt::Display for EvaluateStrategyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown { id } => write!(f, "unknown_strategy: {id}"),
            Self::Dataset(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EvaluateStrategyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unknown { .. } => None,
            Self::Dataset(e) => Some(e.as_ref()),
        }
    }
}

/// Hard cap for replay-style endpoints (indicator + strategy linear walk) — HTTP / batch safety.
const MAX_REPLAY_STEPS: usize = 50_000;

/// Minimum stride ≥ `requested_step` so that walking **`from_idx`…`to_idx`** emits at most
/// **`max_steps`** replay points (`floor(span/step)+1` with `span = to_idx - from_idx`).
fn effective_replay_step(span: usize, requested_step: usize, max_steps: usize) -> usize {
    let requested = requested_step.max(1);
    if span == 0 {
        return requested;
    }
    let max_span_per_step = max_steps.saturating_sub(1).max(1);
    let min_for_cap = span.div_ceil(max_span_per_step).max(1);
    requested.max(min_for_cap)
}

const DEFAULT_SYNTHETIC_BAR_COUNT: u32 = 512;
const DEFAULT_SYNTHETIC_START_MS: i64 = 1_700_000_000_000;
const MAX_SYNTHETIC_BARS: usize = 500_000;

struct EvaluationContext {
    config: StrategyConfig,
    dataset: PreparedDataset,
}

fn build_machine_capabilities() -> MachineCapabilities {
    MachineCapabilities {
        machine_name: "binance_BTC_machine".to_string(),
        machine_version: env!("CARGO_PKG_VERSION").to_string(),
        execution_enabled: false,
        supported_actions: vec!["stand_aside".to_string(), "arm_long_stop".to_string()],
        accepted_inputs: vec![
            "candles".to_string(),
            "synthetic_series".to_string(),
            "bundled_btcusd_1m".to_string(),
            "macro_events_numeric".to_string(),
            "symbol_filters".to_string(),
            "runtime_state_numeric".to_string(),
        ],
    }
}

/// Stateless per [`MachineRequest`]: evaluation uses only this immutable base config plus the
/// request body (no server-side session). Safe to share behind [`std::sync::Arc`] across tasks
/// and **horizontal replicas** (use identical env for the same logical machine).
#[derive(Clone, Debug)]
pub struct DecisionMachine {
    base_config: StrategyConfig,
    capabilities: Arc<MachineCapabilities>,
}

impl Default for DecisionMachine {
    fn default() -> Self {
        Self::new(StrategyConfig::default())
    }
}

impl DecisionMachine {
    pub fn new(base_config: StrategyConfig) -> Self {
        Self {
            base_config,
            capabilities: Arc::new(build_machine_capabilities()),
        }
    }

    /// Metadata built once in [`DecisionMachine::new`] (clone for JSON responses if needed).
    pub fn capabilities(&self) -> &MachineCapabilities {
        self.capabilities.as_ref()
    }

    /// Static discovery payload (also served as `GET /v1/catalog`).
    ///
    /// The catalog is built once on first call (JSON serialization of a sample frame +
    /// BTreeMap flatten) and cached for the lifetime of the process.  Subsequent calls
    /// clone the pre-built `String`-based entries — no JSON round-trip.
    pub fn catalog() -> crate::catalog::CatalogResponse {
        static CATALOG: std::sync::LazyLock<crate::catalog::CatalogResponse> =
            std::sync::LazyLock::new(crate::catalog::build_catalog_response);
        CATALOG.clone()
    }

    /// Merge `request` into this machine's base [`StrategyConfig`] and build a [`PreparedDataset`]
    /// (same path as indicator evaluation). For strategy decisions, pair with
    /// [`crate::strategies::strategy_engine_for`].
    pub fn prepare_dataset(
        &self,
        request: MachineRequest,
    ) -> Result<(StrategyConfig, PreparedDataset)> {
        merge_request_and_build_dataset(&self.base_config, request)
    }

    /// Flatten the **last** prepared bar and return [`IndicatorValueReport`] for a **single** catalog path.
    ///
    /// `indicator_path` must match a leaf key exactly (same string as in `GET /v1/catalog` → `indicators[].path`).
    pub fn evaluate_indicator(
        &self,
        indicator_path: &str,
        request: MachineRequest,
    ) -> Result<IndicatorEvaluateResponse, EvaluateIndicatorError> {
        let ctx = self
            .build_evaluation_context(request)
            .map_err(EvaluateIndicatorError::Dataset)?;
        let index = ctx.dataset.frames.len().checked_sub(1).ok_or_else(|| {
            EvaluateIndicatorError::Dataset(anyhow::anyhow!(
                "at least one closed candle is required"
            ))
        })?;
        let frame = &ctx.dataset.frames[index];
        let v =
            serde_json::to_value(frame).map_err(|e| EvaluateIndicatorError::Dataset(e.into()))?;
        let mut flat = BTreeMap::new();
        crate::catalog::flatten_object_leaves("", &v, &mut flat);
        let value = flat
            .get(indicator_path)
            .ok_or_else(|| EvaluateIndicatorError::Unknown {
                path: indicator_path.to_string(),
            })?;
        let bars_available = ctx.dataset.frames.len();
        let report = indicator_value_report(indicator_path, value, bars_available, &ctx.config);
        Ok(IndicatorEvaluateResponse {
            path: indicator_path.to_string(),
            report,
        })
    }

    /// Run the merged **`strategy_id`** on the **last** closed bar (same failed-acceptance replay
    /// prelude as [`Self::evaluate_strategy_replay`] at that index).
    pub fn evaluate_strategy(
        &self,
        request: MachineRequest,
    ) -> Result<StrategyEvaluateResponse, EvaluateStrategyError> {
        let halt = request.runtime_state.halt_new_entries_flag != 0;
        let mode = if halt {
            SystemMode::Halted
        } else {
            SystemMode::Active
        };
        let ctx = self
            .build_evaluation_context(request)
            .map_err(EvaluateStrategyError::Dataset)?;
        let last = ctx.dataset.frames.len().checked_sub(1).ok_or_else(|| {
            EvaluateStrategyError::Dataset(anyhow::anyhow!(
                "at least one closed candle is required"
            ))
        })?;
        if !supported_strategy_ids().contains(&ctx.config.strategy_id.as_str()) {
            return Err(EvaluateStrategyError::Unknown {
                id: ctx.config.strategy_id.clone(),
            });
        }
        let mut engine =
            strategy_engine_for(&ctx.config).map_err(EvaluateStrategyError::Dataset)?;
        engine.set_system_mode(mode);
        engine.replay_failed_acceptance_window(0, last, &ctx.dataset);
        let decision = engine.decide(last, &ctx.dataset);
        Ok(StrategyEvaluateResponse {
            strategy_id: ctx.config.strategy_id.clone(),
            decision,
        })
    }

    /// Replay one or more indicator dot-paths across a bar window.
    ///
    /// `paths` must be non-empty; each string must match an exact catalog dot-path. Unknown paths
    /// are reported per-step in `unknown_paths` (not an error) so callers can detect typos.
    pub fn evaluate_indicator_replay(
        &self,
        paths: &[&str],
        req: IndicatorReplayRequest,
    ) -> Result<IndicatorReplayResponse, EvaluateIndicatorError> {
        if paths.is_empty() {
            return Err(EvaluateIndicatorError::Dataset(anyhow::anyhow!(
                "at least one indicator path is required"
            )));
        }
        let requested_step = req.step.unwrap_or(1).max(1);
        let ctx = self
            .build_evaluation_context(req.machine)
            .map_err(EvaluateIndicatorError::Dataset)?;
        let bar_count = ctx.dataset.frames.len();
        let last = bar_count.checked_sub(1).ok_or_else(|| {
            EvaluateIndicatorError::Dataset(anyhow::anyhow!(
                "at least one closed candle is required"
            ))
        })?;
        let (from_idx, to_idx) = resolve_replay_window_indices(
            &ctx.dataset.frames,
            last,
            req.from_index,
            req.to_index,
            req.replay_from.as_deref(),
            req.replay_to.as_deref(),
        )
        .map_err(EvaluateIndicatorError::Dataset)?;
        if from_idx > to_idx {
            return Err(EvaluateIndicatorError::Dataset(anyhow::anyhow!(
                "from_index ({from_idx}) must be <= to_index ({to_idx}) after clamping to bar_count-1 ({last})"
            )));
        }
        if from_idx >= bar_count {
            return Err(EvaluateIndicatorError::Dataset(anyhow::anyhow!(
                "from_index ({from_idx}) must be < bar_count ({bar_count})"
            )));
        }
        let span = to_idx - from_idx;
        let step = effective_replay_step(span, requested_step, MAX_REPLAY_STEPS);
        if step > requested_step {
            tracing::warn!(
                requested_step,
                effective_step = step,
                span,
                from_idx,
                to_idx,
                "replay step raised to satisfy MAX_REPLAY_STEPS"
            );
        }
        let estimated = span / step + 1;
        debug_assert!(estimated <= MAX_REPLAY_STEPS);

        let mut steps = Vec::with_capacity(estimated);
        let mut i = from_idx;
        while i <= to_idx {
            let frame = &ctx.dataset.frames[i];
            let close_time = frame.candle.close_time;
            let v = serde_json::to_value(frame)
                .map_err(|e| EvaluateIndicatorError::Dataset(e.into()))?;
            let mut flat = BTreeMap::new();
            crate::catalog::flatten_object_leaves("", &v, &mut flat);
            let mut indicators = BTreeMap::new();
            let mut unknown_paths = Vec::new();
            for &path in paths {
                match flat.get(path) {
                    Some(value) => {
                        let report = indicator_value_report(path, value, bar_count, &ctx.config);
                        indicators.insert(path.to_string(), report);
                    }
                    None => unknown_paths.push(path.to_string()),
                }
            }
            steps.push(IndicatorReplayStep {
                bar_index: i,
                close_time,
                indicators,
                unknown_paths,
            });
            i = i.saturating_add(step);
        }
        Ok(IndicatorReplayResponse { steps })
    }

    /// Walk **`from_index`…`to_index`** (inclusive) by **`step`**, running the configured
    /// [`StrategyConfig::strategy_id`] at each bar (fresh engine per step so failed-acceptance state
    /// matches a forward-only replay from bar 0).
    pub fn evaluate_strategy_replay(
        &self,
        req: StrategyReplayRequest,
    ) -> Result<StrategyReplayResponse, EvaluateStrategyError> {
        let halt = req.machine.runtime_state.halt_new_entries_flag != 0;
        let mode = if halt {
            SystemMode::Halted
        } else {
            SystemMode::Active
        };
        let requested_step = req.step.unwrap_or(1).max(1);
        let ctx = self
            .build_evaluation_context(req.machine)
            .map_err(EvaluateStrategyError::Dataset)?;
        let bar_count = ctx.dataset.frames.len();
        let last = bar_count.checked_sub(1).ok_or_else(|| {
            EvaluateStrategyError::Dataset(anyhow::anyhow!(
                "at least one closed candle is required"
            ))
        })?;
        let (from_idx, to_idx) = resolve_replay_window_indices(
            &ctx.dataset.frames,
            last,
            req.from_index,
            req.to_index,
            req.replay_from.as_deref(),
            req.replay_to.as_deref(),
        )
        .map_err(EvaluateStrategyError::Dataset)?;
        if from_idx > to_idx {
            return Err(EvaluateStrategyError::Dataset(anyhow::anyhow!(
                "from_index ({from_idx}) must be <= to_index ({to_idx}) after clamping to bar_count-1 ({last})"
            )));
        }
        if from_idx >= bar_count {
            return Err(EvaluateStrategyError::Dataset(anyhow::anyhow!(
                "from_index ({from_idx}) must be < bar_count ({bar_count})"
            )));
        }
        let span = to_idx - from_idx;
        let step = effective_replay_step(span, requested_step, MAX_REPLAY_STEPS);
        if step > requested_step {
            tracing::warn!(
                requested_step,
                effective_step = step,
                span,
                from_idx,
                to_idx,
                "strategy replay step raised to satisfy MAX_REPLAY_STEPS"
            );
        }
        let estimated = span / step + 1;
        debug_assert!(estimated <= MAX_REPLAY_STEPS);

        let strategy_id = ctx.config.strategy_id.clone();
        let mut steps = Vec::with_capacity(estimated);
        let mut i = from_idx;
        while i <= to_idx {
            let mut engine =
                strategy_engine_for(&ctx.config).map_err(EvaluateStrategyError::Dataset)?;
            engine.set_system_mode(mode);
            engine.replay_failed_acceptance_window(0, i, &ctx.dataset);
            let decision = engine.decide(i, &ctx.dataset);
            let close_time = ctx.dataset.frames[i].candle.close_time;
            steps.push(StrategyReplayStep {
                bar_index: i,
                close_time,
                decision,
            });
            i = i.saturating_add(step);
        }
        Ok(StrategyReplayResponse { strategy_id, steps })
    }

    fn build_evaluation_context(&self, request: MachineRequest) -> Result<EvaluationContext> {
        let (config, dataset) = merge_request_and_build_dataset(&self.base_config, request)?;
        Ok(EvaluationContext { config, dataset })
    }
}

/// Inclusive UTC calendar days by each bar's **`close_time`**: \[`replay_from` 00:00, `replay_to` end\].
fn replay_day_range_to_indices(
    frames: &[PreparedCandle],
    replay_from: &str,
    replay_to: &str,
) -> Result<(usize, usize)> {
    let from_d = NaiveDate::parse_from_str(replay_from.trim(), "%Y-%m-%d")
        .with_context(|| format!("replay_from {replay_from:?} (expected YYYY-MM-DD UTC)"))?;
    let to_d = NaiveDate::parse_from_str(replay_to.trim(), "%Y-%m-%d")
        .with_context(|| format!("replay_to {replay_to:?} (expected YYYY-MM-DD UTC)"))?;
    if to_d < from_d {
        bail!("replay_to must be on or after replay_from");
    }
    let start = Utc
        .with_ymd_and_hms(from_d.year(), from_d.month(), from_d.day(), 0, 0, 0)
        .single()
        .context("replay_from midnight")?;
    let to_next = to_d.succ_opt().context("replay_to day overflow")?;
    let end_exclusive = Utc
        .with_ymd_and_hms(to_next.year(), to_next.month(), to_next.day(), 0, 0, 0)
        .single()
        .context("replay_to end bound")?;

    let mut from_idx = None;
    let mut to_idx = None;
    for (i, fr) in frames.iter().enumerate() {
        let ct = fr.candle.close_time;
        if ct >= start && ct < end_exclusive {
            if from_idx.is_none() {
                from_idx = Some(i);
            }
            to_idx = Some(i);
        }
    }
    match (from_idx, to_idx) {
        (Some(a), Some(b)) if a <= b => Ok((a, b)),
        _ => bail!(
            "no bars with close_time in UTC [{replay_from} 00:00, {replay_to} 23:59:59.999] inclusive"
        ),
    }
}

fn resolve_replay_window_indices(
    frames: &[PreparedCandle],
    last: usize,
    from_index: Option<usize>,
    to_index: Option<usize>,
    replay_from: Option<&str>,
    replay_to: Option<&str>,
) -> Result<(usize, usize)> {
    match (replay_from, replay_to) {
        (Some(fd), Some(td)) => replay_day_range_to_indices(frames, fd, td),
        (None, None) => {
            let from_idx = from_index.unwrap_or(0);
            let mut to_idx = to_index.unwrap_or(last);
            if to_idx > last {
                to_idx = last;
            }
            Ok((from_idx, to_idx))
        }
        _ => bail!(
            "set both replay_from and replay_to (YYYY-MM-DD UTC), or neither and use from_index/to_index"
        ),
    }
}

fn bar_interval_label_to_ms(label: &str) -> Option<u64> {
    let t = label.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let (num, mult): (&str, u64) = if let Some(rest) = lower.strip_suffix('m') {
        (rest, 60 * 1000)
    } else if let Some(rest) = lower.strip_suffix('h') {
        (rest, 3600 * 1000)
    } else if let Some(rest) = lower.strip_suffix('d') {
        (rest, 86_400 * 1000)
    } else if let Some(rest) = lower.strip_suffix('w') {
        (rest, 7 * 86_400 * 1000)
    } else {
        return None;
    };
    let n: u64 = num.parse().ok()?;
    (n > 0).then_some(n.checked_mul(mult)?)
}

fn count_synthetic_bars(spec: &SyntheticSeries, step_ms: i64) -> Result<usize> {
    if let Some(c) = spec.bar_count {
        if c == 0 {
            return Err(anyhow!("synthetic_series.bar_count must be >= 1"));
        }
        let n = usize::try_from(c).map_err(|_| anyhow!("bar_count too large"))?;
        if n > MAX_SYNTHETIC_BARS {
            return Err(anyhow!(
                "synthetic_series.bar_count {c} exceeds cap {MAX_SYNTHETIC_BARS}"
            ));
        }
        return Ok(n);
    }
    if let Some(end_ms) = spec.end_close_ms {
        let start_ms = spec.start_close_ms.unwrap_or(DEFAULT_SYNTHETIC_START_MS);
        if end_ms < start_ms {
            return Err(anyhow!(
                "synthetic_series.end_close_ms must be >= start_close_ms"
            ));
        }
        let span = i128::from(end_ms - start_ms);
        let step = i128::from(step_ms);
        let n128 = span / step + 1;
        let cap = i128::try_from(MAX_SYNTHETIC_BARS).unwrap_or(i128::MAX);
        if n128 < 1 || n128 > cap {
            return Err(anyhow!(
                "synthetic time range produces an invalid bar count (cap {MAX_SYNTHETIC_BARS})"
            ));
        }
        return usize::try_from(n128).map_err(|_| anyhow!("bar count overflow"));
    }
    let n = usize::try_from(DEFAULT_SYNTHETIC_BAR_COUNT).unwrap();
    if n > MAX_SYNTHETIC_BARS {
        return Err(anyhow!("internal synthetic default exceeds cap"));
    }
    Ok(n)
}

fn build_synthetic_candles(
    spec: &SyntheticSeries,
    bar_interval: &Option<String>,
) -> Result<Vec<Candle>> {
    let step_ms_i64 = spec
        .bar_step_ms
        .map(|u| i64::try_from(u).context("bar_step_ms does not fit i64"))
        .transpose()?
        .or_else(|| {
            bar_interval
                .as_deref()
                .and_then(bar_interval_label_to_ms)
                .and_then(|u| i64::try_from(u).ok())
        })
        .ok_or_else(|| {
            anyhow!(
                "synthetic_series needs `bar_step_ms` or a parseable top-level `bar_interval` (e.g. \"15m\", \"1h\")"
            )
        })?;
    if step_ms_i64 <= 0 {
        return Err(anyhow!("bar_step_ms must be > 0"));
    }
    let start_ms = spec.start_close_ms.unwrap_or(DEFAULT_SYNTHETIC_START_MS);
    let n = count_synthetic_bars(spec, step_ms_i64)?;
    let _ = Utc
        .timestamp_millis_opt(start_ms)
        .single()
        .ok_or_else(|| anyhow!("invalid start_close_ms"))?;

    let mut out = Vec::with_capacity(n);
    let mut price = 50_000.0_f64;
    for i in 0..n {
        let ms = start_ms
            .checked_add(
                i64::try_from(i)
                    .context("bar index")?
                    .checked_mul(step_ms_i64)
                    .ok_or_else(|| anyhow!("synthetic timestamp overflow"))?,
            )
            .ok_or_else(|| anyhow!("synthetic timestamp overflow"))?;
        let close_time = Utc
            .timestamp_millis_opt(ms)
            .single()
            .ok_or_else(|| anyhow!("invalid synthetic bar timestamp at index {i}"))?;
        let open = price;
        price += 10.0;
        let close = price;
        let high = close + 5.0;
        let low = open - 3.0;
        let vol = 100.0 + i as f64;
        let buy_v = vol * 0.62;
        let sell_v = vol - buy_v;
        out.push(Candle {
            close_time,
            open,
            high,
            low,
            close,
            volume: vol,
            buy_volume: Some(buy_v),
            sell_volume: Some(sell_v),
            delta: None,
        });
    }
    Ok(out)
}

fn resolve_candles(
    candles: Vec<Candle>,
    synthetic: Option<&SyntheticSeries>,
    bundled: Option<&BundledBtcUsd1m>,
    bar_interval: &Option<String>,
) -> Result<Vec<Candle>> {
    let n_src = (!candles.is_empty() as u8) + synthetic.is_some() as u8 + bundled.is_some() as u8;
    if n_src > 1 {
        return Err(anyhow!(
            "choose exactly one data source: non-empty `candles`, `synthetic_series`, or `bundled_btcusd_1m`"
        ));
    }
    if !candles.is_empty() {
        return Ok(candles);
    }
    if let Some(b) = bundled {
        return load_btcusd_1m(b);
    }
    if let Some(spec) = synthetic {
        return build_synthetic_candles(spec, bar_interval);
    }
    Err(anyhow!(
        "empty `candles`: provide bars, `synthetic_series`, or `bundled_btcusd_1m`"
    ))
}

fn merge_request_and_build_dataset(
    base_config: &StrategyConfig,
    request: MachineRequest,
) -> Result<(StrategyConfig, PreparedDataset)> {
    let MachineRequest {
        candles,
        bar_interval,
        macro_events,
        runtime_state: _,
        account_equity: _,
        symbol_filters,
        config_overrides,
        synthetic_series,
        bundled_btcusd_1m,
    } = request;

    let candles = resolve_candles(
        candles,
        synthetic_series.as_ref(),
        bundled_btcusd_1m.as_ref(),
        &bar_interval,
    )?;

    let mut config = base_config.clone();

    if let Some(filters) = symbol_filters {
        config = config.with_symbol_filters(filters);
    }

    if let Some(ov) = &config_overrides {
        if let Some(v) = ov.min_target_move_pct {
            config.min_target_move_pct = v;
        }
        if let Some(v) = ov.stop_atr_multiple {
            config.stop_atr_multiple = v;
        }
        if let Some(v) = ov.target_atr_multiple {
            config.target_atr_multiple = v;
        }
        if let Some(v) = ov.runway_lookback {
            config.runway_lookback = v;
        }
        if let Some(v) = ov.ema_fast_period {
            config.ema_fast_period = v;
        }
        if let Some(v) = ov.ema_slow_period {
            config.ema_slow_period = v;
        }
        if let Some(v) = ov.low_vol_enabled {
            config.low_vol_enabled = v;
        }
        if let Some(v) = ov.high_vol_ratio {
            config.high_vol_ratio = v;
        }
        if let Some(v) = ov.breakout_lookback {
            config.breakout_lookback = v;
        }
        if let Some(v) = ov.failed_acceptance_lookback_bars {
            config.failed_acceptance_lookback_bars = v;
        }
        if let Some(v) = ov.trend_confirm_bars {
            config.trend_confirm_bars = v;
        }
        if let Some(v) = ov.vp_enabled {
            config.vp_enabled = v;
        }
        if let Some(v) = ov.vp_lookback_bars {
            config.vp_lookback_bars = v;
        }
        if let Some(v) = ov.vp_value_area_ratio {
            config.vp_value_area_ratio = v;
        }
        if let Some(v) = ov.vp_bin_count {
            config.vp_bin_count = v;
        }
        if let Some(v) = ov.strategy_id.as_ref() {
            config.strategy_id = v.clone();
        }
        if let Some(v) = ov.vwap_anchor_mode {
            config.vwap_anchor_mode = v;
        }
        if let Some(v) = ov.vwap_rolling_bars {
            config.vwap_rolling_bars = Some(v);
        }
        if let Some(v) = ov.higher_tf_factor {
            config.higher_tf_factor = v;
        }
    }

    let dataset = PreparedDataset::build(&config, candles, macro_events)?;
    Ok((config, dataset))
}

fn indicator_value_report(
    path: &str,
    value: &JsonValue,
    bars_available: usize,
    config: &StrategyConfig,
) -> IndicatorValueReport {
    let min = crate::catalog::min_bars_required_for_path(path, config);
    let path_note = crate::catalog::path_note(path);
    let has_signal = !value.is_null();
    let warmup_ok = min
        .map(|m| bars_available >= usize::try_from(m).unwrap_or(usize::MAX))
        .unwrap_or(true);
    let computable = warmup_ok && has_signal;
    IndicatorValueReport {
        value: value.clone(),
        computable,
        min_bars_required: min,
        bars_available,
        path_note,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::{DecisionMachine, MachineRequest, RuntimeState};
    use crate::domain::Candle;

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
    fn evaluate_indicator_matches_single_catalog_path() {
        let machine = DecisionMachine::default();
        let base_time = Utc
            .with_ymd_and_hms(2026, 4, 15, 0, 15, 0)
            .single()
            .expect("time");
        let candles: Vec<Candle> = (0..96)
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
            .collect();

        let req = MachineRequest {
            candles,
            bar_interval: None,
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: Some(100_000.0),
            symbol_filters: None,
            config_overrides: None,
            synthetic_series: None,
            bundled_btcusd_1m: None,
        };

        let out = machine
            .evaluate_indicator("ema_fast", req)
            .expect("indicator");
        assert_eq!(out.path, "ema_fast");
        assert!(out.report.computable);
        assert_eq!(out.report.bars_available, 96);
    }

    #[test]
    fn evaluate_indicator_unknown_path_errors() {
        let machine = DecisionMachine::default();
        let base_time = Utc
            .with_ymd_and_hms(2026, 4, 15, 0, 15, 0)
            .single()
            .expect("time");
        let candles: Vec<Candle> = (0..96)
            .map(|index| Candle {
                close_time: base_time + Duration::minutes(15 * index as i64),
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 10.0,
                buy_volume: Some(6.0),
                sell_volume: Some(4.0),
                delta: None,
            })
            .collect();
        let req = MachineRequest {
            candles,
            bar_interval: None,
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: None,
            symbol_filters: None,
            config_overrides: None,
            synthetic_series: None,
            bundled_btcusd_1m: None,
        };
        let err = machine
            .evaluate_indicator("not_a_catalog_leaf", req)
            .expect_err("unknown");
        assert!(matches!(err, super::EvaluateIndicatorError::Unknown { .. }));
    }

    #[test]
    fn machine_request_accepts_candles_alias() {
        let json = r#"{
            "candles": [],
            "macro_events": [],
            "runtime_state": {"realized_net_r_today": 0.0, "halt_new_entries_flag": 0},
            "account_equity": null,
            "symbol_filters": null,
            "config_overrides": null
        }"#;
        let parsed: MachineRequest = serde_json::from_str(json).expect("parse");
        assert!(parsed.candles.is_empty());
    }

    #[test]
    fn machine_request_minimal_json() {
        let json = r#"{"candles": []}"#;
        let parsed: MachineRequest = serde_json::from_str(json).expect("parse");
        assert!(parsed.candles.is_empty());
        assert!(parsed.macro_events.is_empty());
        assert!(parsed.account_equity.is_none());
        assert!(parsed.symbol_filters.is_none());
        assert!(parsed.config_overrides.is_none());
    }

    #[test]
    fn evaluate_indicator_from_synthetic_series() {
        use super::{DecisionMachine, MachineRequest, RuntimeState, SyntheticSeries};

        let machine = DecisionMachine::default();
        let req = MachineRequest {
            candles: Vec::new(),
            bar_interval: Some("15m".to_string()),
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: None,
            symbol_filters: None,
            config_overrides: None,
            synthetic_series: Some(SyntheticSeries {
                bar_step_ms: None,
                start_close_ms: None,
                end_close_ms: None,
                bar_count: Some(120),
            }),
            bundled_btcusd_1m: None,
        };
        let out = machine
            .evaluate_indicator("ema_fast", req)
            .expect("synthetic ema_fast");
        assert_eq!(out.report.bars_available, 120);
        assert!(out.report.computable);
    }

    #[test]
    fn merge_rejects_candles_plus_synthetic() {
        use chrono::{TimeZone, Utc};

        use super::{DecisionMachine, MachineRequest, RuntimeState, SyntheticSeries};
        use crate::domain::Candle;

        let machine = DecisionMachine::default();
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let one = Candle {
            close_time: t,
            open: 1.0,
            high: 2.0,
            low: 0.5,
            close: 1.5,
            volume: 10.0,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        };
        let req = MachineRequest {
            candles: vec![one],
            bar_interval: Some("15m".to_string()),
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: None,
            symbol_filters: None,
            config_overrides: None,
            synthetic_series: Some(SyntheticSeries {
                bar_step_ms: None,
                start_close_ms: None,
                end_close_ms: None,
                bar_count: Some(10),
            }),
            bundled_btcusd_1m: None,
        };
        let err = machine.evaluate_indicator("ema_fast", req).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("candles") && msg.contains("synthetic_series"),
            "{msg}"
        );
    }

    #[test]
    fn evaluate_strategy_replay_linear_window() {
        use chrono::{Duration, TimeZone, Utc};

        use super::{DecisionMachine, MachineRequest, RuntimeState, StrategyReplayRequest};
        use crate::config::StrategyConfig;
        use crate::domain::Candle;

        let config = StrategyConfig {
            vwma_lookback: 4,
            vol_baseline_lookback_bars: 4,
            vp_enabled: false,
            ..Default::default()
        };
        let machine = DecisionMachine::new(config);
        let base = Utc
            .with_ymd_and_hms(2026, 4, 15, 0, 15, 0)
            .single()
            .unwrap();
        let candles: Vec<Candle> = (0..24)
            .map(|i| Candle {
                close_time: base + Duration::minutes(15 * i as i64),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.5 + i as f64 * 0.2,
                volume: 10.0,
                buy_volume: Some(6.0),
                sell_volume: Some(4.0),
                delta: None,
            })
            .collect();
        let req = StrategyReplayRequest {
            machine: MachineRequest {
                candles,
                bar_interval: None,
                macro_events: Vec::new(),
                runtime_state: RuntimeState::default(),
                account_equity: None,
                symbol_filters: None,
                config_overrides: None,
                synthetic_series: None,
                bundled_btcusd_1m: None,
            },
            from_index: Some(20),
            to_index: Some(22),
            step: Some(1),
            replay_from: None,
            replay_to: None,
        };
        let out = machine
            .evaluate_strategy_replay(req)
            .expect("strategy replay");
        assert_eq!(out.steps.len(), 3);
        assert_eq!(out.steps[0].bar_index, 20);
        assert_eq!(out.steps[2].bar_index, 22);
    }

    #[test]
    fn evaluate_strategy_last_bar_matches_replay_terminal() {
        use chrono::{Duration, TimeZone, Utc};

        use super::{DecisionMachine, MachineRequest, RuntimeState, StrategyReplayRequest};
        use crate::config::StrategyConfig;
        use crate::domain::Candle;

        let config = StrategyConfig {
            vwma_lookback: 4,
            vol_baseline_lookback_bars: 4,
            vp_enabled: false,
            ..Default::default()
        };
        let machine = DecisionMachine::new(config);
        let base = Utc
            .with_ymd_and_hms(2026, 4, 15, 0, 15, 0)
            .single()
            .unwrap();
        let candles: Vec<Candle> = (0..24)
            .map(|i| Candle {
                close_time: base + Duration::minutes(15 * i as i64),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.5 + i as f64 * 0.2,
                volume: 10.0,
                buy_volume: Some(6.0),
                sell_volume: Some(4.0),
                delta: None,
            })
            .collect();
        let machine_req = MachineRequest {
            candles: candles.clone(),
            bar_interval: None,
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: None,
            symbol_filters: None,
            config_overrides: None,
            synthetic_series: None,
            bundled_btcusd_1m: None,
        };
        let last_eval = machine
            .evaluate_strategy(machine_req.clone())
            .expect("strategy evaluate");
        let replay = machine
            .evaluate_strategy_replay(StrategyReplayRequest {
                machine: machine_req,
                from_index: None,
                to_index: None,
                step: Some(1),
                replay_from: None,
                replay_to: None,
            })
            .expect("strategy replay");
        let terminal = replay.steps.last().expect("steps");
        assert_eq!(last_eval.strategy_id, replay.strategy_id);
        assert_eq!(
            serde_json::to_string(&last_eval.decision).expect("serde"),
            serde_json::to_string(&terminal.decision).expect("serde"),
        );
    }

    #[test]
    fn indicator_replay_replay_from_to_utc_days_overrides_indices() {
        use chrono::Duration;

        use super::{DecisionMachine, IndicatorReplayRequest, MachineRequest, RuntimeState};
        use crate::domain::Candle;

        let machine = DecisionMachine::default();
        let base = Utc
            .with_ymd_and_hms(2026, 4, 15, 0, 15, 0)
            .single()
            .unwrap();
        let candles: Vec<Candle> = (0..100)
            .map(|i| Candle {
                close_time: base + Duration::minutes(15 * i as i64),
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 10.0,
                buy_volume: Some(6.0),
                sell_volume: Some(4.0),
                delta: None,
            })
            .collect();

        let req = IndicatorReplayRequest {
            machine: MachineRequest {
                candles,
                bar_interval: None,
                macro_events: Vec::new(),
                runtime_state: RuntimeState::default(),
                account_equity: None,
                symbol_filters: None,
                config_overrides: None,
                synthetic_series: None,
                bundled_btcusd_1m: None,
            },
            from_index: Some(0),
            to_index: Some(2),
            step: Some(1),
            replay_from: Some("2026-04-15".to_string()),
            replay_to: Some("2026-04-15".to_string()),
            indicators: Vec::new(),
        };
        let out = machine
            .evaluate_indicator_replay(&["ema_fast"], req)
            .expect("replay by day");
        assert_eq!(out.steps.len(), 95);
        assert_eq!(out.steps[0].bar_index, 0);
        assert_eq!(out.steps[94].bar_index, 94);
    }
}
