# BTC Continuation V1 Machine

Pure Rust decision module for the BTC `15m` long-only continuation strategy.

The crate is intentionally a thinking machine:

- normalized market/context data goes in
- algorithmic decision output comes out
- no broker connectivity
- no order execution
- no backtest runner
- no file-based runtime contract in the core API

## Core Boundary

The public entrypoint is [src/machine.rs](src/machine.rs).

The canonical JSON in/out contract is documented in
[docs/MACHINE_SCHEMA.md](docs/MACHINE_SCHEMA.md).

`DecisionMachine` accepts:

- closed `15m` candles
- optional macro events
- optional symbol filters
- optional runtime state such as realized `R` for the day
- optional RustyFish daily report

## What "Normalized" Means

In this repo, `normalized` means the input has already been translated into the
machine's canonical internal shape before it reaches the decision engine.

That means:

- timestamps are already converted to RFC3339 / UTC-compatible values
- field names already match the machine schema
- numbers are already parsed as numbers, not strings
- semantic categories are already mapped to numeric codes
- candles are already ordered oldest to newest
- candles are already closed bars, not partial live bars
- venue-specific naming has already been mapped to the canonical names
- text reports have already been reduced to a bounded structured overlay

The current machine boundary is stricter than that:

- request payload values should be numeric-only JSON values
- timestamps should be sent as Unix milliseconds
- macro events should be sent as numeric event codes
- runtime booleans should be sent as numeric flags
- RustyFish context should be sent as numeric overlay fields only

Examples of normalized inputs:

- a candle with numeric `close_time`, `open`, `high`, `low`, `close`, `volume`
- a macro event with numeric `event_time` and numeric `class` code
- symbol filters with `tick_size` and `lot_step`
- a RustyFish overlay with numeric fields like `trend_bias`, `chop_bias`,
  `vol_bias`, `conviction`

Examples of non-normalized inputs:

- Binance raw payloads with exchange-specific field names
- CSV rows where prices and volumes are still strings
- timestamps in string or mixed formats that have not been standardized
- free-text news summaries
- human-written RustyFish prose reports
- partial or unsorted candle streams

The rule is:

- messy, raw, exchange-specific, or human-readable data stays outside the
  machine
- only validated machine-shaped data is allowed inside

So the full pipeline should be:

- raw source data
- parser / adapter / LLM interpreter if needed
- validated normalized schema
- `DecisionMachine`

It returns:

- `MachineAction`
- `SignalDecision`
- optional `PositionPlan`
- diagnostics including the latest prepared frame and effective config

The machine currently emits only two actions:

- `StandAside`
- `ArmLongStop`

That is deliberate. This crate decides. Something else may execute or ignore the decision.

## Example

```rust
use btc_continuation_v1::{DecisionMachine, MachineRequest, RuntimeState};
use btc_continuation_v1::domain::Candle;
use chrono::{Duration, TimeZone, Utc};

let machine = DecisionMachine::default();
let base_time = Utc.with_ymd_and_hms(2026, 4, 15, 0, 15, 0).single().unwrap();

let candles_15m: Vec<Candle> = (0..96)
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

let response = machine.evaluate(MachineRequest {
    candles_15m,
    macro_events: Vec::new(),
    runtime_state: RuntimeState::default(),
    account_equity: Some(100_000.0),
    symbol_filters: None,
    rustyfish_report: None,
})?;
```

## Architecture

Core strategy:

- `src/strategy/gates/`
  One file per veto/gate.

- `src/strategy/formulas/`
  One file per tunable formula family.

- `src/strategy/prepare.rs`
  Feature preparation from raw candles.

- `src/strategy/engine/evaluation.rs`
  Decision evaluation only.

Context:

- `src/context/overlay.rs`
  Bounded overlay type.

- `src/context/policy.rs`
  Clamp policy for external context.

- `src/context/rustyfish/`
  RustyFish report contract and mapping.

Input adapters:

- `src/adapters/binance/kline_csv.rs`
  Parses Binance kline CSV payloads from strings into normalized candles.

- `src/adapters/binance/exchange_info.rs`
  Parses Binance `exchangeInfo` JSON payloads from strings into symbol filters.

## What Was Removed

This crate no longer owns:

- backtesting
- trade replay
- PnL accounting
- fees and slippage modeling
- exchange execution
- CLI execution paths
- file-based core inputs

If you want those later, they should live outside this machine as separate modules or services.
