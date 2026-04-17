# Machine Schema

Canonical JSON contract for the BTC continuation decision machine.

This document describes the in/out format you should send if this crate is
wrapped behind an API service.

## Conventions

- Payload format: JSON
- Request payload values: numeric JSON values only
- Timestamps: Unix milliseconds
  Example: `1776341700000`
- Numbers: JSON numbers, not strings
- Optional fields may be omitted or set to `null`

## Request

Top-level shape:

```json
{
  "candles_15m": [],
  "macro_events": [],
  "runtime_state": {
    "realized_net_r_today": 0.0,
    "halt_new_entries_flag": 0
  },
  "account_equity": 100000.0,
  "symbol_filters": {
    "tick_size": 0.1,
    "lot_step": 0.001
  },
  "rustyfish_overlay": {
    "report_timestamp_ms": 1776297600000,
    "trend_bias": 0.4,
    "chop_bias": 0.2,
    "vol_bias": 0.1,
    "conviction": 0.6
  }
}
```

### `candles_15m`

Required. Array of closed `15m` candles ordered from oldest to newest.

Each candle:

```json
{
  "close_time": 1776341700000,
  "open": 84125.4,
  "high": 84210.1,
  "low": 84080.0,
  "close": 84198.6,
  "volume": 154.33,
  "buy_volume": 91.12,
  "sell_volume": 63.21,
  "delta": null
}
```

Rules:

- `close_time` must be the close time of the candle, not the open time
- candles must be closed bars only
- candles should be continuous and sorted ascending by `close_time`
- `delta` is optional
- if `delta` is missing, the machine will infer it from `buy_volume - sell_volume`

### `macro_events`

Optional.

Each event:

```json
{
  "event_time": 1776342600000,
  "class": 1
}
```

Supported `class` codes:

- `1` = CPI
- `2` = Core CPI
- `3` = PPI
- `4` = NFP
- `5` = Unemployment Rate
- `6` = Core PCE
- `7` = GDP Advance
- `8` = FOMC Rate Decision
- `9` = Powell Press Conference

Unknown codes are rejected at deserialization time.

### `runtime_state`

Optional. If omitted, defaults to:

```json
{
  "realized_net_r_today": 0.0,
  "halt_new_entries_flag": 0
}
```

Fields:

- `realized_net_r_today`
  Current realized `R` for the day. If it is already below the daily limit, the
  machine will halt new entries.

- `halt_new_entries_flag`
  Manual hard stop from the orchestrator. Use `0` for false, non-zero for true.

### `account_equity`

Optional.

If present, the machine will include `risk_budget_usd` and `qty_btc` in the
`PositionPlan`. If absent, it still returns trigger / stop / target / risk
geometry, but leaves capital-based sizing fields as `null`.

### `symbol_filters`

Optional.

```json
{
  "tick_size": 0.1,
  "lot_step": 0.001
}
```

Use this when the caller wants venue-specific price and size rounding.

### `rustyfish_overlay`

Optional.

```json
{
  "report_timestamp_ms": 1776297600000,
  "trend_bias": 0.4,
  "chop_bias": 0.2,
  "vol_bias": 0.1,
  "conviction": 0.6
}
```

This is numeric daily context only. It does not create trades. It only skews
bounded strategy parameters before evaluation.

## Response

Top-level shape:

```json
{
  "action": "arm_long_stop",
  "decision": {},
  "plan": {},
  "diagnostics": {}
}
```

### `action`

One of:

- `stand_aside`
- `arm_long_stop`

This is intent only. It is not an execution command.

### `decision`

```json
{
  "allowed": true,
  "reasons": [],
  "regime": "normal",
  "trigger_price": 84210.2,
  "atr": 325.4
}
```

Fields:

- `allowed`
  Final allow/block result on the latest bar

- `reasons`
  Empty when allowed. When blocked, contains veto/gate labels such as
  `weekend_ban`, `macro_veto`, `high_vol_regime`, `no_runway`

- `regime`
  `normal` or `high`

- `trigger_price`
  Calculated stop-entry trigger if a setup is valid

- `atr`
  ATR value used by the decision logic

### `plan`

Present only when `decision.allowed = true`.

```json
{
  "trigger_price": 84210.2,
  "stop_price": 83559.4,
  "target_price": 85186.4,
  "target_move_pct": 0.01159,
  "risk_fraction": 0.005,
  "risk_budget_usd": 500.0,
  "risk_usd_per_btc": 650.8,
  "qty_btc": 0.768
}
```

Fields:

- `trigger_price`
  Buy-stop trigger

- `stop_price`
  ATR-based stop rounded to venue tick size

- `target_price`
  ATR-based target rounded to venue tick size

- `target_move_pct`
  Expected target move as a decimal fraction

- `risk_fraction`
  Effective risk fraction after any overlay

- `risk_budget_usd`
  Present only when `account_equity` was supplied

- `risk_usd_per_btc`
  Dollar risk per BTC from trigger to stop

- `qty_btc`
  Present only when `account_equity` was supplied

### `diagnostics`

```json
{
  "as_of": 1776341700000,
  "latest_frame": {},
  "effective_config": {},
  "overlay": {}
}
```

Fields:

- `as_of`
  Latest closed `15m` bar time used for evaluation

- `latest_frame`
  Fully prepared feature frame for the latest candle

- `effective_config`
  Final config after symbol filters and RustyFish overlay

- `overlay`
  Present only when RustyFish context was provided

## `latest_frame` Shape

```json
{
  "candle": {
    "close_time": 1776341700000,
    "open": 84125.4,
    "high": 84210.1,
    "low": 84080.0,
    "close": 84198.6,
    "volume": 154.33,
    "buy_volume": 91.12,
    "sell_volume": 63.21,
    "delta": null
  },
  "ema_fast_15m": 84091.4,
  "ema_slow_15m": 83982.8,
  "ema_fast_1h": 83884.1,
  "ema_slow_1h": 83540.7,
  "vwma_15m": 83920.3,
  "atr_15m": 325.4,
  "atr_pct": 0.00386,
  "atr_pct_baseline": 0.00242,
  "vol_ratio": 1.59,
  "cvd_ema3": 224.7,
  "cvd_ema3_slope": 18.9
}
```

All indicator fields may be `null` if history is insufficient.

## `effective_config` Shape

```json
{
  "vol_baseline_lookback_bars": 960,
  "high_vol_ratio": 1.8,
  "daily_loss_limit_r": -2.0,
  "risk_fraction": 0.005,
  "min_target_move_pct": 0.0075,
  "tick_size": 0.1,
  "lot_step": 0.001,
  "ema_fast_period": 9,
  "ema_slow_period": 21,
  "atr_period": 14,
  "vwma_lookback": 96,
  "trend_confirm_bars": 3,
  "breakout_lookback": 20,
  "runway_lookback": 40,
  "stop_atr_multiple": 2.0,
  "target_atr_multiple": 3.0,
  "low_vol_enabled": true
}
```

## `overlay` Shape

```json
{
  "source_code": 1,
  "report_timestamp_ms": 1776297600000,
  "risk_fraction_multiplier": 0.91,
  "high_vol_ratio_multiplier": 0.98,
  "min_target_move_pct_multiplier": 1.08
}
```

## Capabilities Response

If you expose `DecisionMachine::capabilities()` through an API endpoint, the
JSON shape is:

```json
{
  "machine_name": "btc_continuation_v1_machine",
  "machine_version": "0.1.0",
  "execution_enabled": false,
  "supported_actions": [
    "stand_aside",
    "arm_long_stop"
  ],
  "accepted_inputs": [
    "normalized_15m_candles",
    "macro_events_numeric",
    "symbol_filters",
    "runtime_state_numeric",
    "rustyfish_overlay_numeric"
  ]
}
```
