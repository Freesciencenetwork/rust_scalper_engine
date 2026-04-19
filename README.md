# rust_scalper_engine

## What this is
A Rust engine that takes closed BTC candles (JSON) and returns a decision:

- **stand_aside** → Do nothing. No trade conditions met or risk filters block entry.
- **arm_long_stop** → Prepare a long trade by placing a stop-entry above price (execution handled outside this engine).

It does NOT execute trades.

---

## Core idea

Input:   candles + optional context  
Engine:  computes indicators + applies strategy rules  
Output:  decision JSON (action + metadata)

---

## Quick start

### Run demo
cargo run --bin paper_bot  
→ Runs a local simulation using historical candles and prints decisions.

### Run server
cargo run --bin server  
→ Starts HTTP API to query the engine programmatically.

Optional:
VOL_BASELINE_LOOKBACK_BARS=96 cargo run  
→ Reduces required history for faster testing.

Server:
http://127.0.0.1:8080

---

## Minimal usage

### 1. Fetch candles
```bash
binance-fetch klines --symbol BTCUSDT --interval 15m --limit 1000 > request.json
```
→ Closed OHLCV rows (any timeframe). Add **`"bar_interval": "15m"`** (or your interval label) to the JSON if you edit by hand. **`candles_15m`** is also accepted as a backward-compat alias for **`candles`**.

### 2. Evaluate
```bash
curl -sS -X POST http://127.0.0.1:8080/v1/evaluate \
  -H "Content-Type: application/json" \
  -d @request.json
```
→ Decision for the **last** bar in **`candles`**.

### 3. Response (shape)
```json
{
  "action": "arm_long_stop",
  "decision": { "allowed": true }
}
```

- **action** → **`stand_aside`** or **`arm_long_stop`**.
- **decision.allowed** → whether entry is permitted under current rules.

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

**Evaluate (POST)** — use a real **`request.json`** with enough bars for your config (dev: **`VOL_BASELINE_LOOKBACK_BARS=96`** on the server).
```bash
curl -sS -X POST http://127.0.0.1:8080/v1/evaluate \
  -H 'Content-Type: application/json' \
  -d @request.json

curl -sS -X POST http://127.0.0.1:8080/v1/evaluate/replay \
  -H 'Content-Type: application/json' \
  -d @request.json

curl -sS -X POST http://127.0.0.1:8080/v1/evaluate/multi \
  -H 'Content-Type: application/json' \
  -d @request.json

curl -sS -X POST 'http://127.0.0.1:8080/v1/indicators/ema_fast' \
  -H 'Content-Type: application/json' \
  -d @request.json
```

If **`EVALUATE_API_KEY`** is set on the server, add **`-H "X-Api-Key: $EVALUATE_API_KEY"`** to every **POST** above.

---

## API overview

All routes return JSON. POST routes require `Content-Type: application/json`. Auth (when `EVALUATE_API_KEY` is set) applies only to POST routes — GET discovery routes are always public.

---

### Discovery (GET, no auth, no body)

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
  "accepted_inputs": ["candles", "macro_events_numeric", ...]
}
```

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

### Evaluate (POST, auth required when key is set)

All evaluate endpoints accept the standard [Input format](#input-format) body.

#### `POST /v1/evaluate`
Run the strategy on your candles and return a decision for the **last bar**.

- **Input:** standard body — `candles`, optional context.
- **Output:** `action` (`stand_aside` or `arm_long_stop`), `decision` (gate breakdown), optional `plan` (entry price, stop, target, size), `diagnostics` (config used, indicator snapshot of last bar).

```json
{
  "action": "arm_long_stop",
  "decision": { "allowed": true, "gates": {...} },
  "plan": { "entry": 84500.0, "stop": 83900.0, "target": 85300.0, "qty": 0.01 },
  "diagnostics": { "as_of": 1744676100000, "effective_config": {...}, "latest_frame": {...} }
}
```

#### `POST /v1/evaluate/replay`
Run the strategy across a **window of bars** in a single request — like a mini walk-forward without N separate calls. Returns one entry per bar in the range.

- **Extra body fields** (all optional):
  - `from_index` — first bar index (default `0`)
  - `to_index` — last bar index (default: last bar)
  - `step` — emit every Nth bar (default `1`)

```json
{
  "steps": [
    { "bar_index": 90, "close_time": 1744676100000, "action": "stand_aside", "decision": {...}, "plan": null },
    { "bar_index": 91, "close_time": 1744676100000, "action": "arm_long_stop", "decision": {...}, "plan": {...} }
  ]
}
```

#### `POST /v1/evaluate/multi`
Evaluate **multiple strategies** and/or return a filtered **indicator snapshot** for the last bar — all in one request. Useful when comparing strategy outputs or pulling specific indicator values alongside a decision.

- **Extra body fields** (all optional):
  - `strategies` — list of strategy IDs to run (e.g. `["default","macd_trend"]`; empty = default only)
  - `indicators` — list of dot-paths to include in the response (e.g. `["ema_fast","indicator_snapshot.momentum.rsi_14"]`; empty = all)

```json
{
  "as_of": 1744676100000,
  "bar_index": 95,
  "strategies": {
    "default":    { "action": "arm_long_stop", "decision": {...}, "plan": {...} },
    "macd_trend": { "action": "stand_aside",   "decision": {...}, "plan": null }
  },
  "indicators": {
    "ema_fast": { "value": 84210.5, "computable": true, "min_bars_required": 9, "bars_available": 96 },
    "indicator_snapshot.momentum.rsi_14": { "value": 58.3, "computable": true, ... }
  }
}
```

---

### Indicator compute (POST, auth required when key is set)

These run indicator math on your candle data without running a full strategy evaluation.

#### `POST /v1/indicators/{name}`
Compute a single indicator for the **last bar** of your candles. `{name}` is the exact dot-path from `GET /v1/catalog` (e.g. `ema_fast`, `indicator_snapshot.momentum.rsi_14`).

- **Input:** standard body — `candles` + optional context. No extra fields needed.
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
Compute one indicator across a **bar range** — same windowing as `evaluate/replay`.

- **Extra body fields** (all optional): `from_index`, `to_index`, `step`
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

- **Required extra body field:** `indicators` — non-empty list of dot-paths (e.g. `["ema_fast","atr","indicator_snapshot.momentum.rsi_14"]`)
- **Extra body fields** (optional): `from_index`, `to_index`, `step`
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

## Input format

```json
{
  "candles": [],
  "runtime_state": {
    "realized_net_r_today": 0,
    "halt_new_entries_flag": 0
  },
  "bar_interval": "15m",
  "macro_events": [],
  "account_equity": 100000,
  "symbol_filters": null,
  "config_overrides": null
}
```

### Field explanations

- **candles** → REQUIRED  
  Array of historical price bars (OHLCV). Must be **closed**, **oldest → newest**. JSON field name may be **`candles`** or **`candles_15m`**.

- **runtime_state** → REQUIRED  
  - **realized_net_r_today** → current PnL in risk units  
  - **halt_new_entries_flag** → block new entries (0 or 1)

#### `bar_interval` (optional string)

**Not a fixed enum** — you can send any UTF-8 label. It does **not** drive math; one JSON row = one step. Labels usually mirror whatever you fetched (e.g. Binance-style intervals).

**Common sub-daily ladder (1m → 1h)** — same idea as typical exchange kline lists:

| Range | Example labels |
|-------|------------------|
| **1m–1h** | **`"1m"`**, **`"3m"`**, **`"5m"`**, **`"15m"`**, **`"30m"`**, **`"1h"`** (or **`"60m"`** if you prefer) |

**Above 1h** (still valid labels): **`"2h"`**, **`"4h"`**, **`"6h"`**, **`"12h"`**, **`"1d"`**, **`"1w"`**, etc. — or **`"custom"`** / omit.

The engine **does not** parse these strings into minutes: series are **uniform steps**, warmup is in **bar counts** (see catalog **`min_bars_required`**). The field is for **your logs** and documentation; **`GET /v1/catalog`** describes **`engine_series_semantics`**.

**Higher-timeframe fields** (e.g. **`ema_fast_higher`** / **`ema_slow_higher`**): how many **base** candles form one aggregated step is **`config_overrides.higher_tf_factor`** (default **4**). With **15m** base bars, **4** rows ≈ one hour; if your base step is **1h**, adjust **`higher_tf_factor`** to match how you think about rollups.

- **macro_events** → Optional  
  Scheduled events (`event_time` + numeric **`class`** code); see [`src/domain.rs`](src/domain.rs).

- **account_equity** → Optional  
  Used when the response includes position sizing hints.

- **symbol_filters** → Optional  
  Exchange constraints (e.g. tick size, lot step).

- **config_overrides** → Optional  
  Strategy and engine parameters (see below); includes **`strategy_id`**, **`higher_tf_factor`**, VWAP options, etc. — full list in [`src/machine.rs`](src/machine.rs) (`ConfigOverrides`).

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

## Config example

"config_overrides": {
  "strategy_id": "default",      → choose strategy
  "ema_fast_period": 12,         → short trend window
  "ema_slow_period": 26,         → long trend window
  "breakout_lookback": 20        → bars used to detect breakouts
}

---

## Errors

400 → invalid input (e.g. not enough candles)  
401 → missing/invalid API key  
404 → unknown indicator or strategy  
422 → malformed JSON  

---

## Project structure

machine.rs        → API types + decision engine  
strategies/       → trading logic implementations  
indicators/       → technical indicators (RSI, EMA, etc.)  
market_data/      → data preparation  
context/          → macro event types and domain context  
server.rs         → HTTP server  

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
    "macro_events": [],
    "runtime_state": {"realized_net_r_today": 0.0, "halt_new_entries_flag": 0},
    "account_equity": 100_000.0,
    "symbol_filters": None,
    "config_overrides": None,
}

req = urllib.request.Request(
    f"{BASE}/v1/evaluate",
    data=json.dumps(payload).encode(),
    headers={"Content-Type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(req, timeout=120) as resp:
    out = json.loads(resp.read().decode())
print(out.get("action"), (out.get("decision") or {}).get("allowed"))
```

**With a file from `binance-fetch`:** `payload = json.load(open("request.json"))` then add or fix **`bar_interval`** if missing.

**With `requests`:** `requests.post(f"{BASE}/v1/evaluate", json=payload, timeout=120)` — same payload.

A fuller CLI helper lives in [`examples/engine_http_client.py`](examples/engine_http_client.py).

---

## Deep docs

context/strategy-basis.md → strategy logic  
context/schema.md → full data definitions  
context/indicator-roadmap.md → indicator coverage  

---