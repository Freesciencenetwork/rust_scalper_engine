# RustyFish Daily Vibe Pipeline

This document defines how a `RustyFish` daily vibe report can be piped into the
BTC continuation machine without letting it rewrite the strategy.

## Design Rule

`RustyFish` is a sidecar context engine, not the core trading engine.

It is allowed to:

- skew a small, approved set of parameters
- reduce risk
- make vetoes easier to trigger
- make low-quality conditions stricter

It is not allowed to:

- create trades by itself
- disable hard vetoes
- remove the weekend ban
- flip the strategy from long-only to shorting
- replace the core entry logic
- replace the stop/target model

## Machine Boundary

The machine is split into three layers:

1. `Core Strategy`
   The canonical `v1` rules, indicators, gates, state machine, and action-plan
   model.

2. `Context Overlay`
   A bounded parameter overlay built from a RustyFish daily report.

3. `Exchange Adapter`
   Binance today, other venues later. Exchange code normalizes market data and
   metadata into internal types.

## Data Flow

```text
RustyFish report JSON
        |
        v
context/rustyfish/io.rs
        |
        v
context/rustyfish/mapper.rs
        |
        v
context/overlay.rs   ->  bounded ParameterOverlay
        |
        v
context/policy.rs    ->  apply_overlay_to_config(base_config, overlay)
        |
        v
StrategyConfig used by the decision machine
```

## Current Implemented Path

The current crate already supports:

- parsing a RustyFish daily JSON payload
- mapping that report into a normalized `ParameterOverlay`
- applying that overlay to a `StrategyConfig`
- passing the adjusted config into the decision machine

The intended caller flow is now:

```text
external orchestrator -> parse JSON payload -> RustyFishDailyReport -> overlay -> machine config
```

## Current RustyFish Report Contract

The current JSON shape is:

```json
{
  "report_date": "2026-04-15",
  "trend_bias": 0.4,
  "chop_bias": 0.7,
  "vol_bias": 0.3,
  "conviction": 0.6,
  "summary": "risk-on trend but still choppy"
}
```

Field meaning:

- `trend_bias`
  Range `[-1, 1]`. Positive means trend conditions are favorable.

- `chop_bias`
  Range `[-1, 1]`. Positive means choppy / low-quality continuation conditions.

- `vol_bias`
  Range `[-1, 1]`. Positive means stressed / unstable volatility.

- `conviction`
  Range `[-1, 1]`. Higher means stronger confidence in the daily directional
  environment.

- `summary`
  Human-readable rationale for audit and debugging.

## Mapping Policy

The current mapping is intentionally small:

- `risk_fraction_multiplier`
- `high_vol_ratio_multiplier`
- `min_target_move_pct_multiplier`

This means RustyFish can currently influence:

1. position risk
2. how easily the system classifies conditions as high-vol
3. how strict the low-vol floor becomes

It cannot change:

- entry formula
- stop multiple
- target multiple
- time stop
- hard veto topology

## Clamp Policy

All RustyFish influence is clamped before it reaches the machine:

- `risk_fraction_multiplier` clamped to `[0.50, 1.00]`
- `high_vol_ratio_multiplier` clamped to `[0.85, 1.15]`
- `min_target_move_pct_multiplier` clamped to `[1.00, 1.40]`

Interpretation:

- RustyFish can make the machine more conservative
- RustyFish cannot turn the machine into a different strategy
- RustyFish cannot lever the system beyond the base `v1` risk policy

## Why This Pipe Is Correct

This architecture preserves:

- strategy identity
- auditability
- modularity
- exchange independence

If Binance is replaced tomorrow, the RustyFish pipe does not change because it
is upstream of venue-specific input normalization and downstream of report
generation.

## Recommended Daily Runtime Sequence

For live or paper trading:

1. `00:00-06:00 UTC`
   RustyFish crunches overnight news, flow, and regime context.

2. RustyFish writes one report:
   `reports/rustyfish/YYYY-MM-DD.json`

3. The decision orchestrator loads:
   - base strategy config
   - exchange metadata
   - latest RustyFish report

4. The machine applies the overlay once for the trading day.

5. All decisions for that day log:
   - base config
   - overlay values
   - report date
   - report summary

## Recommended Future Extension

The next clean step is a stricter trait boundary:

```text
trait ContextOverlayAdapter {
    fn load_overlay(&self, as_of_date: NaiveDate) -> Result<ParameterOverlay>;
}
```

Then:

- `RustyFishOverlayAdapter` becomes one implementation
- a future news model or macro model can become another
- the engine still consumes only `ParameterOverlay`

## Final Recommendation

Use RustyFish as a bounded daily context overlay.

Do not put RustyFish in the intraday signal loop.
Do not let RustyFish change the strategy structure.
Do let RustyFish make the machine more selective when daily conditions are bad.
