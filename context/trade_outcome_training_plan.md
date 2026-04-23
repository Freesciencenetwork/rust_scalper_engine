# Trade Outcome API + Parallel Profitability Training Plan

## Goal

Add an additive API path that can tell us whether a strategy signal would have produced a profitable trade after fees and slippage, then use that trade ledger to train a separate profitability model.

This must **not** replace the current trading flow while we build it. The existing strategy evaluation and model-driven trading path stays available as-is.

---

## Working Rule

- Keep `POST /v1/strategies/{id}` unchanged for live decisioning.
- Keep existing Python training tasks usable while the new profitability path is built.
- Add a **parallel** backtest/export path for training data generation.
- Start with the current long-only execution semantics; do not broaden scope until the first version is stable.
- Use conservative, deterministic execution rules so labels are realistic and reproducible.

---

## Why This Is Needed

Right now the Rust API returns strategy decisions, but not realized trade outcomes.

Current ML training is also based on forward market movement labels, not on realized trade PnL net of costs. That means the model can learn that price moved, while still failing to learn whether the actual trade implementation was worth taking.

We need a bridge between:

1. `signal at bar t`
2. `deterministic simulated execution`
3. `net trade result after fees/slippage`
4. `training label derived from that net result`

---

## Target Architecture

### Track A — Existing Trading Path (unchanged)

- Rust live decision endpoint keeps returning the same decision payload.
- Existing models and current strategy-driven trading remain usable.
- No dependency on the new profitability route for normal trading.

### Track B — New Outcome/Training Path (additive)

- New Rust backtest-style endpoint produces a trade ledger.
- Trade ledger includes gross result, costs, and net result.
- Python pipeline converts that ledger into training rows.
- A new meta-model is trained to decide whether a candidate trade is worth taking.

The practical meaning is:

- base strategy still proposes a trade
- profitability model becomes an optional filter
- if the profitability model is disabled, current behavior remains unchanged

---

## Rust API Work

### 1. Add explicit execution assumptions

Create request-side types for execution cost assumptions, for example:

- `entry_fee_bps`
- `exit_fee_bps`
- `entry_slippage_bps`
- `exit_slippage_bps`
- `stop_extra_slippage_bps`
- `max_hold_bars`
- `same_bar_policy`

Notes:

- First version should assume taker-like costs unless we have exchange-specific fill evidence.
- `same_bar_policy` must be explicit because same-bar target/stop ambiguity will otherwise poison labels.

### 2. Add a dedicated backtest request/response

Add a new request type, likely alongside the existing replay types in `src/machine.rs`.

Suggested shape:

- `StrategyBacktestRequest`
  - flattened `MachineRequest`
  - replay window fields
  - execution assumptions

- `StrategyBacktestResponse`
  - `strategy_id`
  - `summary`
  - `trades`

### 3. Add a trade ledger model

Define a `TradeOutcome` record with enough detail for both debugging and ML labeling.

Minimum fields:

- `signal_bar_index`
- `entry_bar_index`
- `exit_bar_index`
- `signal_close_time`
- `entry_close_time`
- `exit_close_time`
- `entry_price_raw`
- `entry_price_fill`
- `exit_price_raw`
- `exit_price_fill`
- `trigger_price`
- `stop_price`
- `target_price`
- `atr_at_signal`
- `bars_held`
- `exit_reason`
- `gross_return_pct`
- `gross_r`
- `fee_cost_pct`
- `slippage_cost_pct`
- `net_return_pct`
- `net_r`
- `profitable`

Summary fields should include at least:

- `trade_count`
- `win_rate`
- `avg_net_r`
- `profit_factor`
- `expectancy_r`
- `max_drawdown_r`

### 4. Add the route

Add:

- `POST /v1/strategies/{strategy_id}/backtest`

Keep this route separate from:

- `POST /v1/strategies/{strategy_id}`
- `POST /v1/strategies/replay`

Reason:

- live decisioning stays lightweight
- outcome generation can evolve independently
- clients do not need to change existing trading calls

### 5. Define deterministic execution rules

Before coding, lock these rules down in the implementation comments and tests:

- when a signal becomes an order
- when the entry is considered filled
- whether entry happens on trigger touch, next bar open, or another explicit rule
- how stop and target are checked intrabar
- how same-bar stop/target conflicts are resolved
- what happens if neither stop nor target hits
- whether `max_hold_bars` forces an exit
- whether only one position can be open at a time

First version should prefer simple and reproducible over realistic-but-ambiguous.

---

## Python Training Work

### 6. Keep current training scripts intact

Do not break or replace:

- `python_pipeline/train_v2.py`
- current Task A / Task B artifacts
- current inference flow

These remain the baseline while the profitability path is built.

### 7. Add a trade-ledger dataset builder

Create a new pipeline step that:

1. calls the Rust backtest route over historical windows
2. receives the `trades` ledger
3. joins each trade to the feature snapshot at signal time
4. writes a clean table for ML training

Each training row should represent one candidate trade, not one arbitrary bar.

Suggested output columns:

- `strategy_id`
- `timestamp_ms`
- feature columns
- execution assumption columns
- `gross_r`
- `net_r`
- `profitable`
- `exit_reason`
- `bars_held`

### 8. Add profitability labels

Start with two labels:

- classifier label: `take_trade = 1 if net_r > buffer_r else 0`
- regression label: `expected_net_r = net_r`

Notes:

- `buffer_r` should be slightly above zero so the classifier ignores fragile near-breakeven trades.
- Keeping both labels lets us compare “binary filter” vs “expected value ranker”.

### 9. Train the profitability model as a meta-model

Recommended first rollout:

- one profitability model per strategy
- input = feature snapshot at signal time
- label = `take_trade` or `net_r`
- evaluation = walk-forward only

Do **not** optimize this model for plain accuracy.

Optimize for trading metrics:

- expectancy
- profit factor
- average net R
- drawdown
- coverage

### 10. Keep live usage optional

Live execution integration should be phase-based:

- Phase 1: train and evaluate offline only
- Phase 2: log model score in parallel, but do not gate trades
- Phase 3: enable as an optional filter

Fallback rule:

- if the profitability model is unavailable, current strategy behavior remains active

---

## File-Level Plan

### Rust

- `src/machine.rs`
  - add request/response types
  - add trade ledger structs
  - add backtest evaluation function

- `src/bin/server.rs`
  - add `POST /v1/strategies/{strategy_id}/backtest`

- `src/lib.rs`
  - export new public types

- `src/strategy/`
  - add or reuse execution helpers for entry/exit simulation

- `tests/`
  - add unit and scenario coverage for execution math and ledger correctness

### Python

- `python_pipeline/`
  - add a new dataset builder for trade outcomes
  - add a new training script for profitability gating
  - keep existing `train_v2.py` path untouched

### Docs

- `README.md`
  - document the new route once stable

- `context/`
  - keep decisions and state aligned after implementation starts

---

## Validation Plan

### Rust validation

- hand-check a few known trades against manual calculations
- unit-test fee and slippage arithmetic
- unit-test same-bar conflict behavior
- unit-test forced exit behavior
- scenario-test backtest summaries on fixed fixtures

### ML validation

- use chronological walk-forward only
- compare model-vs-no-model on the same candidate trades
- report both gross and net performance
- measure how much trade count drops when filtering

Success is not “high accuracy”.

Success is:

- better expectancy after costs
- acceptable coverage
- lower drawdown or higher profit factor than the unfiltered baseline

---

## Rollout Order

1. Finalize execution assumptions and same-bar policy.
2. Implement `TradeOutcome` and `StrategyBacktestRequest/Response`.
3. Add `POST /v1/strategies/{id}/backtest`.
4. Add Rust tests for deterministic ledger generation.
5. Build Python exporter from backtest response to training table.
6. Train first profitability classifier on one strategy.
7. Compare filtered vs unfiltered walk-forward results.
8. If it improves net expectancy, wire it in as an optional live filter.
9. Repeat per scalping strategy.

---

## Non-Goals For V1

- replacing the current live endpoint
- mixing all strategies into one shared profitability model on day one
- modeling partial fills or order book microstructure in detail
- trying to guarantee “only profitable trades”

We are aiming for:

- better trade selection
- fewer low-quality entries
- positive expected value after realistic costs

Not:

- perfect certainty on each trade

---

## Open Decisions

- exact fee model to assume in training
- whether stop exits get extra slippage vs target exits
- same-bar stop/target precedence
- whether first version should allow only one open trade at a time
- whether the profitability model is per-strategy only or later pooled
- whether V1 label should be `net_r > 0`, `net_r > buffer_r`, or direct `net_r` regression

---

## Recommended First Slice

The fastest defensible first slice is:

1. implement `POST /v1/strategies/{id}/backtest`
2. emit a deterministic `TradeOutcome` ledger with net-of-cost fields
3. export one strategy’s candidate trades into a training table
4. train a simple binary profitability filter
5. compare filtered vs unfiltered results offline

This gives us a real answer on whether the model can improve trade selection without disrupting the current trading path.

---

## Evaluation — Claude Sonnet 4.6

### What's solid

**Architecture fit is good.** `StrategyBacktestRequest` follows the exact same pattern as `StrategyReplayRequest` (flattened `MachineRequest` + window fields), so the Rust implementation path is clear. The new route `/v1/strategies/{id}/backtest` correctly stays separate from the live and replay routes.

**The ML motivation is correct.** Task B has MCC 0.15 but that's directional signal — not realized PnL signal. The gap between "price moved in your direction" and "your actual trade made money after fees and slippage" is real. This is the right problem to solve next.

**Phase-based live integration is appropriate** — offline first, then parallel logging, then optional gating. Good instinct given the decaying MCC fold-by-fold (0.35 → 0.05).

**Walk-forward-only for the profitability model** is correctly specified.

---

### Issues to resolve before coding

**1. Open Decisions contradict the Working Rules**

The plan says "lock down deterministic execution rules before coding" but then defers the same-bar stop/target policy, one-position-at-a-time constraint, and forced-exit exit price to the Open Decisions list. These need to be settled first, not during implementation. Suggested defaults:

- Same-bar policy: stop wins (conservative; protects capital)
- One position at a time: yes (long-only, simplest)
- Max-hold forced exit: fill at that bar's close price

**2. Max-hold forced exit price is unspecified**

The plan says "`max_hold_bars` forces an exit" but not at what price. For 1m scalping this matters — close vs next-bar open produces different net_r. Pick one and put it in the execution rules, not the open decisions.

**3. Entry fill mechanics aren't connected to `SignalDecision`**

The current strategy emits `arm_long_stop` decisions with a trigger price. The backtest needs to simulate: at which future bar does the candle's low touch the trigger? The plan doesn't specify how to read the trigger price from the existing `SignalDecision` struct and how many look-ahead bars to check. This needs explicit rules before implementation.

**4. Python builder calls the HTTP API for historical data**

Step 7 proposes calling `POST /v1/strategies/{id}/backtest` across historical windows from Python. With 7.5M 1m candles, chunking and making HTTP calls will be slow and adds network overhead. A more efficient path: expose the backtest logic as a Rust CLI that writes a parquet ledger directly (similar to `fetch_max_btcusdt_1m.rs`). The Python builder can then read the parquet instead of making HTTP calls.

**5. Feature join may silently lose warmup bars**

The join between `signal_close_time` and the indicators parquet (`indicators_full.parquet`) will drop rows where features are null (warmup period). The plan doesn't mention filtering these out or how to handle them. If warmup rows sneak into training they'll produce NaN-poisoned feature rows.

**6. `machine.rs` is already 1321 lines**

Putting the backtest logic there adds significant complexity to an already large file. Consider `src/backtest.rs` with a `BacktestEngine` that mirrors `DecisionMachine`'s pattern.

---

### ML concerns

**Trade count is unknown.** The profitability model trains on one row per `arm_long_stop` signal. If the strategy fires infrequently, the training set could be too small for walk-forward to work. Before committing to this approach, run the replay endpoint on all historical data for one strategy and count actual signal bars.

**Label choice matters more than the plan implies.** The plan says keep both `take_trade` (binary) and `net_r` (regression) and decide later. But the choice affects which model architecture and metrics to use. Recommendation: start with the binary classifier at `net_r > 0.3R` (not zero — near-zero trades don't help and add noise). MCC is still the right headline metric.

**Profitability model evaluated on wrong metric if you use accuracy.** The plan correctly says "optimize for expectancy, profit factor, coverage" — make sure `evaluate.py` has those metrics ready before training, or the walk-forward results won't tell you what you need to know.

---

### Rollout order adjustment

The current rollout has step 1 as "finalize execution assumptions" but the Open Decisions section shows those aren't finalized. Recommended order:

1. **Settle all Open Decisions** — write them into the plan as locked rules, not options
2. Count actual signal frequency on historical data for one strategy
3. Then implement `TradeOutcome` + `StrategyBacktestRequest/Response`
4. Consider Rust CLI export instead of HTTP-based Python builder
5. Rest of plan follows as written

---

### Summary

The plan is architecturally sound and solves a real problem. The blocking issues are (a) unresolved execution assumptions that are marked as both required-before-coding and still-open, and (b) the Python data pipeline performance concern for large historical datasets. Settle those first, then the implementation path is clear.

---

## Quick Readback Of Sonnet's Reply

1. The overall architecture is good.
   `StrategyBacktestRequest` plus a separate `POST /v1/strategies/{id}/backtest` route fits the current API cleanly, and the profitability-model idea is solving the right problem.

2. The plan has a contradiction before coding.
   It says execution rules must be locked first, but still leaves key items open: `same_bar_policy`, `one position at a time`, and `max_hold_bars` exit behavior.

3. It wants these defaults fixed up front.
   `same-bar: stop wins`, `one position at a time: yes`, `forced max-hold exit: current bar close`.

4. It flagged a missing detail in entry simulation.
   The backtest must explicitly define how an `arm_long_stop` decision becomes an actual fill from the existing `trigger_price`.

5. It thinks HTTP is the wrong bulk path for training export.
   For large history, it recommends a Rust CLI that writes a parquet trade ledger, instead of Python calling the HTTP route repeatedly.

6. It warned about feature joins dropping warmup rows.
   Joining trade times to feature parquet can silently lose rows or introduce NaNs unless warmup handling is explicit.

7. It suggested not putting all of this into `src/machine.rs`.
   That file is already large; it recommended a separate `src/backtest.rs` or `BacktestEngine`.

8. On ML, it said to check signal count first.
   Before committing, count how many actual candidate trades one strategy produces over full history.

9. It recommended starting with a binary profitability label.
   Specifically: start with something like `net_r > 0.3R`, not just `net_r > 0`.

10. It said evaluation must be trading-metric-first.
    Use expectancy, profit factor, and coverage, not plain accuracy.

Bottom line:

The plan is solid, but we should settle the execution assumptions and the export approach first, then implementation is straightforward.

---

## Execution Plan

This is the concrete implementation plan derived from the above design and evaluation. Every open decision is resolved here. Each step has the exact files to touch, what to add, and the acceptance criteria.

---

### Locked Execution Rules (V1)

These were "Open Decisions" above. They are now settled.

| Rule | Decision | Rationale |
|---|---|---|
| Same-bar stop/target conflict | **Stop wins** | Conservative; prevents PnL inflation from ambiguous bars |
| One position at a time | **Yes** | Long-only V1; avoids overlapping-position bookkeeping |
| Max-hold forced exit price | **Bar's close price** | Deterministic, no look-ahead, matches how 1m data is timestamped |
| Entry fill rule | **Next bar after signal**: fill at `trigger_price` if next bar's high ≥ `trigger_price`, else signal expires | Simple, no partial fills, no multi-bar look-ahead |
| Entry look-ahead | **1 bar** (signal expires if next bar doesn't trigger) | Keeps logic tight; multi-bar look-ahead is a V2 option |
| Stop price | `trigger_price - stop_atr_multiple × atr` (existing `build_position_plan` formula) | Already implemented |
| Target price | `trigger_price + target_atr_multiple × atr` (existing `build_position_plan` formula) | Already implemented |
| Fee model | **Taker both sides**: `entry_fee_bps = 10`, `exit_fee_bps = 10` (Binance spot taker) | Conservative default; overridable per request |
| Slippage model | `entry_slippage_bps = 2`, `exit_slippage_bps = 2`, `stop_extra_slippage_bps = 3` | Stops get extra slippage because market orders on adverse moves are worse |
| Max hold bars | **20 bars** default, overridable per request | For 1m scalping, 20 minutes is already generous |
| R-multiple definition | `1R = trigger_price - stop_price` (risk per unit, same as `risk_usd_per_btc` in existing `PositionPlan`) | Standard |

---

### Step 0 — Measure Signal Frequency

**Goal:** Confirm there are enough candidate trades to train a profitability model before building the whole pipeline.

**Action:** Run the existing strategy replay on bundled BTC 1m history for the `default` strategy. Count how many bars have `decision.allowed == true`.

**How:**
```bash
# Use a short script or curl against the running server:
# POST /v1/strategies/replay with bundled_btcusd_1m: { "all": true }
# Count steps where decision.allowed == true
```

**Accept if:** ≥ 5,000 candidate trades across full history (need enough for 5-fold walk-forward with ≥ 200 trades per test fold).

**Fail-fast:** If < 1,000 candidates across all history, the profitability model will be undertrained. Revisit plan scope before proceeding.

---

### Step 1 — `src/backtest.rs`: Types and Simulation Logic

**New file.** Keep `machine.rs` at its current size.

#### 1a — Execution assumptions struct

```rust
// src/backtest.rs

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionAssumptions {
    #[serde(default = "default_entry_fee_bps")]
    pub entry_fee_bps: f64,        // default 10.0
    #[serde(default = "default_exit_fee_bps")]
    pub exit_fee_bps: f64,         // default 10.0
    #[serde(default = "default_entry_slippage_bps")]
    pub entry_slippage_bps: f64,   // default 2.0
    #[serde(default = "default_exit_slippage_bps")]
    pub exit_slippage_bps: f64,    // default 2.0
    #[serde(default = "default_stop_extra_slippage_bps")]
    pub stop_extra_slippage_bps: f64, // default 3.0
    #[serde(default = "default_max_hold_bars")]
    pub max_hold_bars: usize,      // default 20
}
```

#### 1b — Trade outcome struct

```rust
#[derive(Clone, Debug, Serialize)]
pub struct TradeOutcome {
    pub signal_bar_index: usize,
    pub entry_bar_index: usize,
    pub exit_bar_index: usize,
    pub signal_close_time: DateTime<Utc>,
    pub entry_close_time: DateTime<Utc>,
    pub exit_close_time: DateTime<Utc>,
    pub entry_price_raw: f64,      // trigger_price
    pub entry_price_fill: f64,     // trigger + slippage
    pub exit_price_raw: f64,       // stop/target/close price before slippage
    pub exit_price_fill: f64,      // after slippage
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    TargetHit,
    StopHit,
    MaxHoldExpired,
}
```

#### 1c — Backtest summary struct

```rust
#[derive(Clone, Debug, Serialize)]
pub struct BacktestSummary {
    pub trade_count: usize,
    pub win_count: usize,
    pub loss_count: usize,
    pub win_rate: f64,
    pub avg_gross_r: f64,
    pub avg_net_r: f64,
    pub profit_factor: f64,   // sum(winning net_r) / abs(sum(losing net_r))
    pub expectancy_r: f64,    // avg_net_r (same thing, named for clarity)
    pub max_drawdown_r: f64,  // peak-to-trough of cumulative net_r
    pub total_net_r: f64,
    pub avg_bars_held: f64,
}
```

#### 1d — Request / response types

```rust
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
```

#### 1e — Core simulation function

```rust
/// Walk bars `from..=to`. At each bar, run the strategy. If `allowed && trigger_price.is_some()`:
///   1. Check if position is already open → skip (one-at-a-time rule)
///   2. Look at bar `signal_idx + 1`: if high >= trigger_price → FILL entry
///   3. Walk forward from entry bar, checking stop/target each bar:
///      - Bar low <= stop_price → StopHit (stop wins on same-bar conflict)
///      - Bar high >= target_price → TargetHit
///      - bars_held >= max_hold_bars → MaxHoldExpired at bar close
///   4. Compute fill prices with slippage, fees, gross/net R
pub fn simulate_backtest(
    config: &StrategyConfig,
    dataset: &PreparedDataset,
    from: usize,
    to: usize,
    exec: &ExecutionAssumptions,
) -> Vec<TradeOutcome>
```

The summary is computed from the `Vec<TradeOutcome>` by a separate `fn compute_summary(trades: &[TradeOutcome]) -> BacktestSummary`.

**Files changed:**
- `src/backtest.rs` — **new** (~300 lines)
- `src/lib.rs` — add `pub mod backtest;` and re-exports

**Tests (in `src/backtest.rs`):**
- `test_entry_fill_on_next_bar` — signal at bar N, bar N+1 high >= trigger → entry
- `test_signal_expires_if_no_trigger` — bar N+1 high < trigger → no trade
- `test_stop_hit` — bar low <= stop → StopHit, fill = stop - slippage
- `test_target_hit` — bar high >= target → TargetHit, fill = target - slippage
- `test_same_bar_stop_wins` — both stop and target touched → StopHit
- `test_max_hold_exit` — neither stop nor target → exit at close after max bars
- `test_fee_and_slippage_arithmetic` — known prices, verify net_r = gross_r - fees - slippage
- `test_one_position_at_a_time` — overlapping signals don't open second position
- `test_summary_profit_factor` — 2 wins at +2R, 1 loss at -1R → PF = 4.0
- `test_summary_max_drawdown` — known sequence, verify drawdown

---

### Step 2 — Wire Backtest Into `DecisionMachine`

**File:** `src/machine.rs`

Add one method to `DecisionMachine`:

```rust
pub fn evaluate_backtest(
    &self,
    req: StrategyBacktestRequest,
) -> Result<StrategyBacktestResponse, EvaluateStrategyError>
```

This method:
1. Calls `self.build_evaluation_context(req.machine)` (existing)
2. Resolves window indices (existing `resolve_replay_window_indices`)
3. Calls `backtest::simulate_backtest(...)` from Step 1
4. Calls `backtest::compute_summary(...)` from Step 1
5. Returns `StrategyBacktestResponse`

**~40 lines added to `machine.rs`.** All heavy logic lives in `backtest.rs`.

**Files changed:**
- `src/machine.rs` — add `evaluate_backtest` method
- `src/lib.rs` — add re-exports for `StrategyBacktestRequest`, `StrategyBacktestResponse`

---

### Step 3 — HTTP Route

**File:** `src/bin/server.rs`

#### 3a — Add route

```rust
// In v1_post Router:
.route("/strategies/{strategy_id}/backtest", post(evaluate_strategy_backtest))
```

#### 3b — Add handler

```rust
async fn evaluate_strategy_backtest(
    State(machine): State<Arc<DecisionMachine>>,
    Path(strategy_id): Path<String>,
    Json(mut request): Json<StrategyBacktestRequest>,
) -> Result<Json<StrategyBacktestResponse>, StrategyApiError> {
    // Inject strategy_id from path (same pattern as evaluate_strategy_last_bar)
    let mut co = request.machine.config_overrides.take().unwrap_or_default();
    co.strategy_id = Some(strategy_id.trim().to_string());
    request.machine.config_overrides = Some(co);
    machine
        .evaluate_backtest(request)
        .map(Json)
        .map_err(StrategyApiError)
}
```

#### 3c — Update route table doc comment

Add to the route table in the module doc:
```
| POST | `/v1/strategies/{id}/backtest` | Backtest: trade ledger with net-of-cost outcomes. |
```

**Files changed:**
- `src/bin/server.rs` — add route, handler, update imports and doc comment (~25 lines)

---

### Step 4 — Smoke Test the Route

Before building the Python pipeline, manually verify the route works.

```bash
cargo run --bin server &
# Wait for "listening"

curl -s http://localhost:8080/v1/strategies/default/backtest \
  -H 'Content-Type: application/json' \
  -d '{
    "bundled_btcusd_1m": { "from": "2024-01-01", "to": "2024-01-31" },
    "execution": { "max_hold_bars": 20 }
  }' | python3 -m json.tool | head -50
```

**Accept if:** Response contains `strategy_id`, `summary`, `trades` array. Spot-check one trade's arithmetic by hand.

---

### Step 5 — Python Trade Ledger Exporter

**New file:** `python_pipeline/export_trade_ledger.py`

This script:
1. Calls `POST /v1/strategies/{id}/backtest` with `bundled_btcusd_1m: { "all": true }`
2. Receives the JSON response
3. Flattens `trades` into a DataFrame
4. Joins each trade row to `features_normalized.parquet` on `signal_close_time ≈ timestamp_ms` (nearest match within 60s tolerance)
5. Drops rows where any feature column is NaN (warmup protection)
6. Writes `data/trade_ledger_{strategy_id}.parquet`

Output columns:
- `strategy_id`, `signal_close_time_ms`
- All 129 feature columns
- `entry_fee_bps`, `exit_fee_bps`, `entry_slippage_bps`, `exit_slippage_bps`
- `gross_r`, `net_r`, `profitable`, `exit_reason`, `bars_held`

**Performance note:** With `bundled_btcusd_1m: { "all": true }` (7.5M rows), this is one HTTP call, not chunked. The server processes it in one pass. If memory is too tight on 16GB, add `replay_from`/`replay_to` chunking (yearly windows).

**Files changed:**
- `python_pipeline/export_trade_ledger.py` — **new** (~120 lines)

---

### Step 6 — Python Profitability Training Script

**New file:** `python_pipeline/train_profitability.py`

#### 6a — Labels

```python
BUFFER_R = 0.3  # ignore fragile near-breakeven trades
df["take_trade"] = (df["net_r"] > BUFFER_R).astype(int)
```

#### 6b — Model

- LightGBM binary classifier (same `LGBM_BINARY_PARAMS` from `config.py`)
- Input: 129 feature columns (same as Task A/B)
- Label: `take_trade`
- Walk-forward: 5 folds, expanding window (reuse `walk_forward.py`)

#### 6c — Evaluation metrics

Must report (beyond MCC):
- **Expectancy**: mean `net_r` of trades where model says `take_trade=1`
- **Profit factor**: sum(winning `net_r`) / abs(sum(losing `net_r`)) for model-selected trades
- **Coverage**: fraction of candidate trades the model keeps
- **Max drawdown (R)**: peak-to-trough of cumulative `net_r` for selected trades

Compare against the unfiltered baseline (all candidate trades taken).

#### 6d — Checkpoint

Save model + metrics to `python_pipeline/models/checkpoints/btc_1m_profitability_{strategy_id}_lgbm_{metric}_{date}/`

**Files changed:**
- `python_pipeline/train_profitability.py` — **new** (~200 lines)
- `python_pipeline/config.py` — add `PROFITABILITY_BUFFER_R = 0.3` and `LGBM_PROFITABILITY_PARAMS` (copy of `LGBM_BINARY_PARAMS`)

**Does NOT touch:** `train_v2.py`, `train.py`, `train_task_a.py`, `train_task_b.py`, or any existing training artifacts.

---

### Step 7 — Evaluate Results

Not code changes — this is the decision gate.

**Run:**
```bash
python3 python_pipeline/export_trade_ledger.py --strategy default
python3 python_pipeline/train_profitability.py --strategy default
```

**Decision matrix:**

| Outcome | Action |
|---|---|
| Model expectancy > unfiltered expectancy, coverage ≥ 40% | Proceed to Phase 2 (parallel live logging) |
| Model expectancy > unfiltered but coverage < 20% | Too aggressive — lower `BUFFER_R`, retrain |
| Model expectancy ≤ unfiltered | Profitability filter doesn't help this strategy — try different features or stop here |
| < 1000 trades in ledger | Data too thin — expand to more strategies or longer history |

---

### File Summary

| File | Action | Lines (est.) |
|---|---|---|
| `src/backtest.rs` | **new** | ~300 |
| `src/machine.rs` | add `evaluate_backtest` | ~40 |
| `src/lib.rs` | add module + re-exports | ~10 |
| `src/bin/server.rs` | add route + handler | ~25 |
| `python_pipeline/export_trade_ledger.py` | **new** | ~120 |
| `python_pipeline/train_profitability.py` | **new** | ~200 |
| `python_pipeline/config.py` | add profitability params | ~10 |
| **Total new/changed** | | **~705** |

**Not touched:** `train_v2.py`, existing models, existing endpoints, existing strategy logic.

---

### Dependency Order

```
Step 0 (signal count)
  │
  ▼
Step 1 (backtest.rs: types + simulation)
  │
  ▼
Step 2 (machine.rs: wire in)
  │
  ▼
Step 3 (server.rs: HTTP route)
  │
  ▼
Step 4 (smoke test)
  │
  ▼
Step 5 (Python exporter)
  │
  ▼
Step 6 (Python training)
  │
  ▼
Step 7 (evaluate → decide)
```

Steps 1–3 can be implemented and tested in one session (Rust side). Steps 5–6 are one session (Python side). Step 4 bridges them.
