# rust_scalper_engine

Decision support for **closed-bar** technical analysis: one Rust library builds a rich [`PreparedDataset`](src/market_data/) from OHLCV, an optional **HTTP server** exposes **discovery**, **indicator** compute/replay, and **linear strategy replay** — all **stateless** JSON, **no HTTP authentication** (run locally or behind your own proxy).

**Not included:** order routing, accounts, or live exchange feeds. Strategy output is **intent** (`stand_aside`, `arm_long_stop`, …), not executed trades.

---

## Quick start

```bash
cargo run
# → http://0.0.0.0:8080  (override with HOST / PORT)
```

| Variable | Role |
|----------|------|
| `HOST` | Bind address (default `0.0.0.0`) |
| `PORT` | TCP port (default `8080`) |
| `VOL_BASELINE_LOOKBACK_BARS` | Shorter vol warmup in dev (e.g. `96`) |
| `EVALUATE_MAX_INFLIGHT` | Optional per-process cap on concurrent indicator + strategy replay POSTs |
| `BTCUSD_1M_CSV` | Override path to bundled **`btcusd_1-min_data.csv`** (default: `src/historical_data/…` under the crate root) |

POST bodies are limited to **10 MiB** JSON.

---

## HTTP API (summary)

All JSON responses use `Content-Type: application/json` except **`GET /health`** (plain `ok`).

| Method | Path | Body | Notes |
|--------|------|------|--------|
| GET | `/health` | — | Liveness |
| GET | `/v1/capabilities` | — | Name, version, `accepted_inputs`, `supported_actions` |
| GET | `/v1/catalog` | — | Strategies + indicators + `indicator_paths` |
| GET | `/v1/indicators` | — | Same indicator list as catalog |
| GET | `/v1/indicators/{path}` | — | One leaf metadata; **404** `unknown_indicator` |
| GET | `/v1/strategies` | — | Strategy ids + descriptions |
| GET | `/v1/strategies/{id}` | — | **404** `unknown_strategy` |
| POST | `/v1/indicators/{path}` | [`MachineRequest`](#request-body-machinejson) | Last-bar value for one catalog path |
| POST | `/v1/indicators/{path}/replay` | Same + `from_index`? `to_index`? `step`? | One indicator over indices |
| POST | `/v1/indicators/replay` | Same + **`indicators`** array | Multi-path replay |
| POST | `/v1/strategies/replay` | Same + optional index window | [`StrategyReplayResponse`](src/machine.rs); **≤ 50 000** steps |

`{path}` must match **`GET /v1/catalog`** exactly (e.g. `ema_fast`, `indicator_snapshot.momentum.rsi_14`).

### Curl one-liners

```bash
curl -sS http://127.0.0.1:8080/v1/catalog | head
curl -sS -X POST 'http://127.0.0.1:8080/v1/indicators/ema_fast' \
  -H 'Content-Type: application/json' -d @request.json
```

Load candles with your own tooling (e.g. `binance-fetch klines … > request.json`). Alias: **`candles_15m`** → same as **`candles`**.

---

## Request body (`MachineRequest`)

Types live in [`src/machine.rs`](src/machine.rs). Every POST above uses the **same root object** (replay routes add fields on the same JSON object).

### Where do the bars come from?

Pick **exactly one** of: non-empty **`candles`**, **`bundled_btcusd_1m`**, or **`synthetic_series`**.

**A — Your series**

```json
{
  "candles": [ … ],
  "bar_interval": "15m"
}
```

**B — Bundled BTC/USD 1-minute CSV** (shipped under [`src/historical_data/`](src/historical_data/); override path with **`BTCUSD_1M_CSV`**). Dates are **UTC calendar days** `YYYY-MM-DD`; **`to`** is inclusive through end of that day.

```json
{
  "bar_interval": "1m",
  "bundled_btcusd_1m": {
    "from": "2012-01-01",
    "to": "2012-01-31"
  }
}
```

| `bundled_btcusd_1m` | Meaning |
|---------------------|--------|
| `from` | Optional lower day (inclusive). Omit to start at first CSV row (unless `all` is set). |
| `to` | Optional upper day (**inclusive**). Omit to read through end of file. |
| `all` | If **`true`**, load from first row of the file (do not set **`from`**/**`to`**). |

Hard cap: **500 000** rows per request — narrow **`from`**/**`to`** if you hit it. Full multi-year 1m files exceed that; use a date range or **`step`** on replay.

**C — Synthetic demo bars** (smoke tests without CSV)

```json
{
  "bar_interval": "15m",
  "synthetic_series": { "bar_count": 120 }
}
```

| `synthetic_series` | Meaning |
|--------------------|--------|
| `bar_step_ms` | Optional; ms between closes. If omitted, a parseable **`bar_interval`** is required (e.g. `"15m"` → 900 000). |
| `start_close_ms` | Optional first close (UTC **ms**); default anchor in code. |
| `end_close_ms` | With `start_close_ms`, inclusive range → bar count from step. Ignored if **`bar_count`** is set. |
| `bar_count` | Exact number of bars. If neither `bar_count` nor `end_close_ms`, default **512** bars. |

Hard cap: **500 000** synthetic bars.

### Replay-only fields (same JSON root)

| Field | Applies to | Default |
|-------|----------------|--------|
| `from_index` | indicator + strategy replay | `0` |
| `to_index` | … | last bar (clamped) |
| `step` | … | `1` (use `>1` to thin long histories) |
| `indicators` | **`POST /v1/indicators/replay` only** | — (required non-empty list of paths) |

### Semantics that confuse people once

- **One JSON row = one engine step.** Spacing follows **`close_time`** in **`candles`** or in the bundled CSV; **`bar_interval`** is mainly a **label** (and drives **synthetic** step when `bar_step_ms` is omitted).
- **“Timeframe” for replay** = the series you chose (`candles`, **`bundled_btcusd_1m`** slice, or synthetic length). There is no `start_year` query for bundled data beyond **`from`**/**`to`**.
- **Higher-TF indicator fields** (e.g. `ema_fast_higher`) use config **`higher_tf_factor`** (default **4** base bars per bucket) unless you set **`config_overrides`** — see `ConfigOverrides` in `machine.rs`.

### Optional / advanced keys

Omit unless you need them: **`macro_events`**, **`runtime_state`** (halt / PnL flags for strategy replay), **`account_equity`**, **`symbol_filters`**, **`config_overrides`** (EMA periods, `strategy_id`, VWAP, `higher_tf_factor`, …). **`GET /v1/capabilities`** lists `accepted_inputs` as strings.

### Candle object

Required: **`close_time`** (ms), **`open`**, **`high`**, **`low`**, **`close`**, **`volume`**. Optional: **`buy_volume`**, **`sell_volume`**, **`delta`**. Oldest → newest, **closed** bars only.

---

## Responses (shapes)

**Last bar (`POST /v1/indicators/{path}`)**

```json
{
  "path": "ema_fast",
  "value": 84210.5,
  "computable": true,
  "min_bars_required": 9,
  "bars_available": 96
}
```

**Indicator replay** — `steps[]` with `bar_index`, `close_time`, per-path reports (and `unknown_paths` on multi replay).

**Strategy replay** — `strategy_id` + `steps[]` with `bar_index`, `close_time`, `decision` (`SignalDecision`: `allowed`, `reasons`, `trigger_price`, …).

---

## Strategy replay details

- Walks strategy **`default`** unless **`config_overrides.strategy_id`** selects another registered id.
- Per emitted index `i`: strategy is wired, **`runtime_state.halt_new_entries_flag`** applied, **`replay_failed_acceptance_window(0, i, …)`**, then **`decide(i, …)`**.
- **≤ 50 000** emitted steps per request; widen **`step`** or split the series if you hit the cap.
- **400** + JSON `invalid_request` on bad windows, cap exceeded, or dataset build failure.

**Rust:** [`DecisionMachine::prepare_dataset`](src/machine.rs), [`evaluate_strategy_replay`](src/machine.rs), or `strategy_engine_for` + manual `decide` — see [`tests/engine_advice.rs`](tests/engine_advice.rs).

---

## Errors

| Status | Typical cause |
|--------|----------------|
| **400** | Invalid window, replay cap, empty `candles` without `synthetic_series`, both `candles` and `synthetic_series`, bad timestamps |
| **404** | Unknown indicator path or strategy id (GET metadata) |
| **422** | Malformed JSON |

---

## Examples (Python)

| Script | Purpose |
|--------|---------|
| [`examples/simple_post.py`](examples/simple_post.py) | `bundled_btcusd_1m` date range + `POST …/ema_fast` (sets `BTCUSD_1M_CSV` if unset) |
| [`examples/engine_http_client.py`](examples/engine_http_client.py) | `--catalog`, indicator POST, `--replay`, `--strategy-replay` |
| [`examples/indicators_replay_date_range.py`](examples/indicators_replay_date_range.py) | Filter `candles` by `close_time`, `POST /v1/indicators/replay` |

---

## Library layout

| Path | Responsibility |
|------|----------------|
| [`src/machine.rs`](src/machine.rs) | `MachineRequest`, `DecisionMachine`, merge + dataset, indicator + replay APIs |
| [`src/market_data/`](src/market_data/) | `PreparedDataset`, `PreparedCandle`, `IndicatorSnapshot` |
| [`src/indicators/`](src/indicators/) | TA implementations |
| [`src/strategies/`](src/strategies/) | `Strategy` engines (`strategy_engine_for`) |
| [`src/bin/server.rs`](src/bin/server.rs) | Axum router |

---

## Security note

There is **no API key** on the server. Use **`127.0.0.1`**, a private network, or a reverse proxy with auth if the port is reachable.

---

## Further reading

- [`context/strategy-basis.md`](context/strategy-basis.md) — default strategy story  
- [`context/schema.md`](context/schema.md) — data definitions  
- [`context/indicator-roadmap.md`](context/indicator-roadmap.md) — indicator coverage  

---

## Diagram (HTTP evaluate)

```mermaid
sequenceDiagram
    participant C as Client
    participant S as server.rs
    participant M as DecisionMachine
    participant P as PreparedDataset

    C->>S: POST /v1/indicators/{path} JSON
    S->>M: evaluate_indicator(path, MachineRequest)
    M->>M: merge_request_and_build_dataset
    M->>P: PreparedDataset::build
    P-->>M: dataset
    M-->>S: IndicatorEvaluateResponse
    S-->>C: JSON
```

**Summary:** The README is reorganized around one **HTTP table**, one **request-body** section (real vs **synthetic** candles + replay fields), compact **response/error** notes, and pointers to examples and `context/` docs — aligned with the current server and `MachineRequest` behavior.
