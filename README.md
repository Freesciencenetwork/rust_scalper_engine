# rust_scalper_engine

## What this is

**Rust library:** Builds a full [`PreparedDataset`](src/market_data/) from closed OHLCV candles, runs technical indicators, and can drive **strategy** code (e.g. default continuation logic with **`stand_aside`** / **`arm_long_stop`** intent — no broker execution).

**HTTP server (`server` binary):** Stateless JSON API for **discovery**, **indicator** compute/replay, and **linear strategy replay** (`POST /v1/strategies/replay`) over a bar index range — including walking **all** candles you send (e.g. years of history), subject to a **50k-step** safety cap per request.

---

## Core idea

| Surface | Input | Output |
|--------|--------|--------|
| **HTTP** | Same [`MachineRequest`](src/machine.rs) body as the library | **Indicators:** `IndicatorEvaluateResponse` / `IndicatorReplayResponse`. **Strategy walk:** `StrategyReplayResponse` from `POST /v1/strategies/replay`. |
| **Library** | `MachineRequest` + `StrategyConfig` | `PreparedDataset`, per-strategy [`SignalDecision`](src/strategy/decision.rs), etc. |

---

## Quick start

### Run server
```bash
cargo run
# same as: cargo run --bin server
```
Starts the HTTP API (default **`HOST`**: `0.0.0.0`, **`PORT`**: `8080`).

**Shorter warmup in dev** (fewer bars needed before vol metrics stabilize):
```bash
VOL_BASELINE_LOOKBACK_BARS=96 cargo run
```

**Base URL:** `http://127.0.0.1:8080` (or your `HOST`/`PORT`).

---

## Minimal usage

### 1. Fetch candles
```bash
binance-fetch klines --symbol BTCUSDT --interval 15m --limit 1000 > request.json
```
Closed OHLCV rows (any timeframe). Add **`"bar_interval": "15m"`** if you edit by hand. **`candles_15m`** is accepted as an alias for **`candles`**.

### 2. Pick an indicator path
```bash
curl -sS http://127.0.0.1:8080/v1/catalog | head
```
Use an exact **`indicators[].path`** string (e.g. **`ema_fast`**, or a nested leaf such as **`indicator_snapshot.momentum.rsi_14`**).

### 3. Compute last bar
```bash
curl -sS -X POST 'http://127.0.0.1:8080/v1/indicators/ema_fast' \
  -H "Content-Type: application/json" \
  -d @request.json
```

### 4. Response (shape)
```json
{
  "path": "ema_fast",
  "value": 84210.5,
  "computable": true,
  "min_bars_required": 9,
  "bars_available": 96
}
```

- **`computable`** → value is non-null **and** (when catalogued) enough bars were supplied for warmup.
- **`path`**, **`min_bars_required`**, **`bars_available`**, optional **`path_note`** — see types in [`src/machine.rs`](src/machine.rs).

---

## Strategy from Rust (not HTTP)

**HTTP:** `POST /v1/strategies/replay` with the same candle JSON (optional `from_index` / `to_index` / `step`) returns [`StrategyReplayResponse`](src/machine.rs).

**In Rust:**

1. [`DecisionMachine::prepare_dataset`](src/machine.rs) — merges `MachineRequest` into config and returns `(StrategyConfig, PreparedDataset)`.
2. Or [`DecisionMachine::evaluate_strategy_replay`](src/machine.rs) — one call for a linear window.
3. Manual path: `strategy_engine_for(&config)` ([`src/strategies/mod.rs`](src/strategies/mod.rs)), then `replay_failed_acceptance_window` and `decide` per bar.

Integration-style reference: [`tests/engine_advice.rs`](tests/engine_advice.rs).

---

## Curl examples

Base URL: **`http://127.0.0.1:8080`** (change with **`HOST`** / **`PORT`**). Discovery routes need no body.

**Liveness & metadata**
```bash
curl -sS http://127.0.0.1:8080/health
curl -sS http://127.0.0.1:8080/v1/capabilities
```

**Discovery**
```bash
curl -sS http://127.0.0.1:8080/v1/catalog
curl -sS http://127.0.0.1:8080/v1/indicators
curl -sS 'http://127.0.0.1:8080/v1/indicators/ema_fast'
curl -sS http://127.0.0.1:8080/v1/strategies
curl -sS 'http://127.0.0.1:8080/v1/strategies/default'
```

**Indicator compute (POST)** — same candle JSON as the library; use enough history for your paths (dev: start server with **`VOL_BASELINE_LOOKBACK_BARS=96`**).
```bash
curl -sS -X POST 'http://127.0.0.1:8080/v1/indicators/ema_fast' \
  -H 'Content-Type: application/json' \
  -d @request.json

# Replay window: flatten MachineRequest + optional from_index, to_index, step
curl -sS -X POST 'http://127.0.0.1:8080/v1/indicators/ema_fast/replay' \
  -H 'Content-Type: application/json' \
  -d @request.json

# Multi-indicator replay: add non-empty "indicators": ["ema_fast","atr", ...] to the body
curl -sS -X POST 'http://127.0.0.1:8080/v1/indicators/replay' \
  -H 'Content-Type: application/json' \
  -d @request.json

# Strategy linear replay: add optional "from_index", "to_index", "step" to the same candle JSON
# (omit them to walk bar 0 → last; cap 50k steps — use a coarser step for very long histories)
curl -sS -X POST 'http://127.0.0.1:8080/v1/strategies/replay' \
  -H 'Content-Type: application/json' \
  -d @request.json
```

The server **does not use HTTP authentication**. Run it on `localhost`, bind to an internal interface only, or put it behind your own reverse proxy / VPN if you need access control.

---

## API overview

All HTTP responses here are **JSON** (`Content-Type: application/json`) except plain **`GET /health`**. POST bodies use **`Content-Type: application/json`**.

---

### Discovery (GET, no body)

#### `GET /health`
Liveness check. Returns `ok` (plain text). Use for load balancer health probes.

#### `GET /v1/capabilities`
Returns engine name, version, whether execution is enabled, and the list of accepted input fields.
```json
{
  "machine_name": "binance_BTC_machine",
  "machine_version": "0.1.0",
  "execution_enabled": false,
  "supported_actions": ["stand_aside", "arm_long_stop"],
  "accepted_inputs": [
    "candles",
    "macro_events_numeric",
    "symbol_filters",
    "runtime_state_numeric"
  ]
}
```

(`supported_actions` describe the **library** strategy contract; the HTTP server only serves indicator compute + discovery.)

#### `GET /v1/catalog`
Full combined discovery payload: all strategy IDs + every indicator dot-path with warmup metadata. Use this once to learn what paths exist before querying individual endpoints.
```json
{
  "strategies": [{ "id": "default", "description": "..." }],
  "indicators": [{ "path": "ema_fast", "min_bars_required": 9 }],
  "indicator_paths": ["ema_fast", "atr", ...]
}
```

#### `GET /v1/indicators`
Array of all indicator entries — same as `catalog.indicators`. Useful when you only need the indicator list without strategy metadata.
```json
[{ "path": "ema_fast", "min_bars_required": 9 }, ...]
```

#### `GET /v1/indicators/{name}`
Metadata for a single indicator by exact dot-path (e.g. `/v1/indicators/ema_fast` or `/v1/indicators/indicator_snapshot.momentum.rsi_14`). Returns `404` with `{"error":"unknown_indicator","path":"..."}` if not found.
```json
{ "path": "ema_fast", "min_bars_required": 9, "path_note": null }
```

#### `GET /v1/strategies`
Array of all strategy entries — same as `catalog.strategies`.
```json
[{ "id": "default", "description": "Long-only 15m BTC continuation (project default)." }, ...]
```

#### `GET /v1/strategies/{id}`
Metadata for one strategy by ID (e.g. `/v1/strategies/default`). Returns `404` with `{"error":"unknown_strategy","id":"..."}` if not found.
```json
{ "id": "default", "description": "Long-only 15m BTC continuation (project default)." }
```

---

### Indicator compute (POST)

**Last-bar POST** (`/v1/indicators/{name}`): body is exactly [`MachineRequest`](#input-format) — at minimum **`candles`**; optional **`bar_interval`**, **`runtime_state`**, and other keys as in that section.

**Replay POST** (`…/replay` and `/v1/indicators/replay`): JSON is the **same fields at the top level** as `MachineRequest` (serde **flattens** the nested struct), **plus** replay controls on the same object:

| Field | Required | Meaning |
|--------|----------|---------|
| `candles` (or `candles_15m`) | yes | Bar series, oldest → newest |
| `bar_interval` | no | **Label only** (e.g. `"15m"`). Same as last-bar: **not parsed into minutes**; see [`bar_interval` (optional string)](#bar_interval-optional-string). |
| `from_index`, `to_index`, `step` | no | Window over **row indices** (defaults: full series, `step` 1) |
| `indicators` | multi replay only | Non-empty list of catalog dot-paths |

**“Timeframe” for math:** the engine treats one JSON row as **one step**; spacing comes from **your** candle timestamps, not from `bar_interval`. For higher-TF rollup fields (e.g. `ema_fast_higher`), set **`config_overrides.higher_tf_factor`** (bar count per bucket), not a string timeframe.

Indicator paths must match `GET /v1/catalog` exactly.

#### `POST /v1/indicators/{name}`
Compute a single indicator for the **last bar** of your candles. `{name}` is the exact dot-path from `GET /v1/catalog` (e.g. `ema_fast`, `indicator_snapshot.momentum.rsi_14`).

- **Input:** `candles` + optional context (including optional **`bar_interval`** for your own documentation).
- **Output:** the path, its value, whether it was computable (enough bars), warmup requirements.
- **404** if the path is not in the catalog.

```json
{
  "path": "ema_fast",
  "value": 84210.5,
  "computable": true,
  "min_bars_required": 9,
  "bars_available": 96
}
```

#### `POST /v1/indicators/{name}/replay`
Compute one indicator across a **bar range**. Body = **replay shape** above: all `MachineRequest` fields **plus** optional `from_index`, `to_index`, `step` (no `indicators` key — the URL carries the path).

**Example request (truncated `candles`):**
```json
{
  "candles": [ … ],
  "bar_interval": "15m",
  "from_index": 80,
  "to_index": 95,
  "step": 1
}
```

- **Output:** one step per bar with the indicator value at that point in the series.

```json
{
  "steps": [
    {
      "bar_index": 50,
      "close_time": 1744676100000,
      "indicators": {
        "ema_fast": { "value": 83900.0, "computable": true, "min_bars_required": 9, "bars_available": 96 }
      }
    }
  ]
}
```

#### `POST /v1/indicators/replay`
Same as `{name}/replay` but for **multiple indicators at once**. The paths are listed in the body instead of the URL.

- **Required:** `indicators` — non-empty list of dot-paths (e.g. `["ema_fast","atr","indicator_snapshot.momentum.rsi_14"]`)
- **Same optional window / label fields:** `from_index`, `to_index`, `step`, and any `MachineRequest` field such as **`bar_interval`**
- Unknown paths appear in `unknown_paths` per step (not an error — lets you detect typos).

```json
{
  "steps": [
    {
      "bar_index": 50,
      "close_time": 1744676100000,
      "indicators": {
        "ema_fast": { "value": 83900.0, "computable": true, ... },
        "atr":      { "value": 310.5,   "computable": true, ... }
      },
      "unknown_paths": []
    }
  ]
}
```

---

### Strategy linear replay (`POST /v1/strategies/replay`)

Walks the configured **strategy** (crate default is **`default`** unless you add optional **`config_overrides`** in JSON) over a **contiguous bar index range** so you can test the same rules you would use live, bar by bar, on a slice of history or on **the entire** `candles` array you POST.

**Replay vs “timeframe” (for your script):** There is **no** separate URL like `/replay/...` and **no** extra query that means “15m only” or “2020–2024 only”. **Your timeframe is whatever you put in `candles`** — e.g. only 15m bars, only 1h bars, or a slice you already filtered in Python/Rust before POSTing. Optional **`bar_interval`** (e.g. `"15m"`, `"1h"`) is a **label for you and your logs**; the engine still steps by **row index** (`0` … `len-1`), not by parsing that string into minutes. **`from_index` / `to_index` / `step`** choose **which rows** of that array get a `decide` call (e.g. walk the whole 10-year series linearly, or only indices 10 000–11 000). To test another TF, POST a different `candles` series (or resample upstream).

**Semantics (matches library-style replay):** for each emitted index `i`, the engine instantiates the strategy, applies **`halt_new_entries_flag`** via [`RuntimeState`](src/machine.rs), runs **`replay_failed_acceptance_window(0, i, …)`**, then **`decide(i, …)`**. One JSON step per bar in the walk.

**Body:** flattened [`MachineRequest`](#input-format) **plus** (all optional):

| Field | Default | Meaning |
|--------|---------|---------|
| `from_index` | `0` | First bar index (inclusive). |
| `to_index` | last bar | Last bar index (inclusive); clamped to dataset length − 1. |
| `step` | `1` | Emit every Nth bar (`≥ 1`). Use e.g. `step: 4` to thin a multi-year 15m series while staying under the cap. |

**Cap:** at most **50 000** emitted steps per request (error if `from_index`…`to_index` with `step` would exceed that — widen `step` or split the series).

**Response shape:**
```json
{
  "strategy_id": "default",
  "steps": [
    {
      "bar_index": 100,
      "close_time": 1744676100000,
      "decision": {
        "allowed": false,
        "reasons": ["macro_veto"],
        "regime": "normal",
        "trigger_price": 84200.0,
        "atr": 310.5
      }
    }
  ]
}
```

**Errors:** `400` + JSON `invalid_request` if the window is invalid, the step cap is exceeded, or the dataset cannot be built.

**In Rust** the same idea is [`DecisionMachine::evaluate_strategy_replay`](src/machine.rs); see also [Strategy from Rust](#strategy-from-rust-not-http) for ad-hoc wiring.

---

## Input format

Minimal POST body (other keys are optional and default in the server — see [`MachineRequest`](src/machine.rs)):

```json
{
  "candles": [],
  "bar_interval": "15m"
}
```

### Field explanations

- **candles** → REQUIRED  
  Array of historical price bars (OHLCV). Must be **closed**, **oldest → newest**. JSON field name may be **`candles`** or **`candles_15m`**.

- **runtime_state** → Optional in JSON (defaults to zeros / no halt). Set it when you care about **halt** or reporting PnL in risk units for **strategy** replay; omit for simple indicator pulls.

#### `bar_interval` (optional string)

**Not a fixed enum** — you can send any UTF-8 label. It does **not** drive math; one JSON row = one step. Labels usually mirror whatever you fetched (e.g. Binance-style intervals).

**Common sub-daily ladder (1m → 1h)** — same idea as typical exchange kline lists:

| Range | Example labels |
|-------|------------------|
| **1m–1h** | **`"1m"`**, **`"3m"`**, **`"5m"`**, **`"15m"`**, **`"30m"`**, **`"1h"`** (or **`"60m"`** if you prefer) |

**Above 1h** (still valid labels): **`"2h"`**, **`"4h"`**, **`"6h"`**, **`"12h"`**, **`"1d"`**, **`"1w"`**, etc. — or **`"custom"`** / omit.

The engine **does not** parse these strings into minutes: series are **uniform steps**, warmup is in **bar counts** (see catalog **`min_bars_required`**). The field is for **your logs** and documentation; **`GET /v1/catalog`** describes **`engine_series_semantics`**.

**Higher-timeframe fields** (e.g. **`ema_fast_higher`**): the server uses built-in defaults (e.g. **`higher_tf_factor` = 4** base bars per rollup) unless you add optional **`config_overrides`** — see [`ConfigOverrides`](src/machine.rs) only when you need to tune that.

**Advanced (optional, skip for BTC replay scripts):** **`macro_events`**, **`account_equity`**, **`symbol_filters`**, **`config_overrides`** exist for richer sessions (macro calendar, sizing, tick/lot overrides, EMA periods / `strategy_id`). For “just replay indicators on my candle file”, **`candles`** (+ replay fields like **`indicators`**) is enough.

---

## Candle schema

{
  "close_time": 1744676100000,
  "open": 100,
  "high": 101,
  "low": 99,
  "close": 100.5,
  "volume": 10
}

Optional:
- buy_volume → aggressive buying volume
- sell_volume → aggressive selling volume
- delta → buy - sell imbalance

---

## Rules (strict)

- candles must be **closed** → no partial data
- ordered **oldest → newest**
- timestamps in **milliseconds**
- indicators are **computed internally** (do not send them)

---

## Errors

400 → invalid input (e.g. not enough candles)  
404 → unknown indicator or strategy id (discovery GETs)  
422 → malformed JSON  

---

## Project structure

`src/machine.rs` → `MachineRequest`, `DecisionMachine` (dataset merge, `prepare_dataset`, indicator + strategy replay)  
`src/strategies/` → strategy engines (`Strategy` trait)  
`src/indicators/` → technical indicators  
`src/market_data/` → `PreparedDataset` / `PreparedCandle`  
`src/context/` → daily overlay and related context helpers  
`src/bin/server.rs` → HTTP server (discovery + indicator POSTs)  

---

## Python example (stdlib, no extra packages)

Same JSON as **`curl`**. For **`bar_interval`**, see **[`bar_interval` (optional string)](#bar_interval-optional-string)** above.

```python
import json
import os
import urllib.request

BASE = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")

def candle_row(i: int, start_ms: int) -> dict:
    step_ms = 15 * 60 * 1000
    t = start_ms + i * step_ms
    o, c = 100.0 + i * 0.1, 100.7 + i * 0.1
    return {
        "close_time": t,
        "open": o,
        "high": c + 0.5,
        "low": o - 0.5,
        "close": c,
        "volume": 10.0 + i,
        "buy_volume": 6.0 + i * 0.1,
        "sell_volume": 4.0 + i * 0.1,
        "delta": None,
    }

start_ms = 1_744_676_100_000
payload = {
    "candles": [candle_row(i, start_ms) for i in range(96)],
    "bar_interval": "15m",
}

req = urllib.request.Request(
    f"{BASE}/v1/indicators/ema_fast",
    data=json.dumps(payload).encode(),
    headers={"Content-Type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(req, timeout=120) as resp:
    out = json.loads(resp.read().decode())
print(out.get("path"), out.get("computable"), out.get("value"))
```

**With a file from `binance-fetch`:** `payload = json.load(open("request.json"))` then add or fix **`bar_interval`** if missing.

**With `requests`:** `requests.post(f"{BASE}/v1/indicators/ema_fast", json=payload, timeout=120)` — same payload shape.

Smallest client: [`examples/simple_post.py`](examples/simple_post.py) (stdlib, one `POST /v1/indicators/ema_fast`). See also [`examples/engine_http_client.py`](examples/engine_http_client.py): **`--catalog`**, default **`POST /v1/indicators/ema_fast`**, **`--replay`**, **`--strategy-replay`**. For **three indicators over 2010–2012**, see [`examples/indicators_replay_date_range.py`](examples/indicators_replay_date_range.py).

---

## Deep docs

context/strategy-basis.md → strategy logic  
context/schema.md → full data definitions  
context/indicator-roadmap.md → indicator coverage  

---