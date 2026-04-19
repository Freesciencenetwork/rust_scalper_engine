# rust_scalper_engine

HTTP server for closed-bar indicators and replay. Routes: [`src/bin/server.rs`](src/bin/server.rs). JSON types: [`src/machine.rs`](src/machine.rs).

## Run

```bash
cargo run
```

**`HOST`** / **`PORT`** (default `0.0.0.0:8080`). No HTTP auth — use **`127.0.0.1`** or a proxy if the port is reachable.

```bash
VOL_BASELINE_LOOKBACK_BARS=96 cargo run   # optional: shorter vol warmup in dev
```

## Test

```bash
cargo test --all-targets --locked
```

## API

**`/v1/*`** responses are JSON except where noted. POST bodies max **10 MiB**.

- **`GET /health`** — no args, no body → plain text **`ok`**
- **`GET /v1/capabilities`** — no args, no body → **`MachineCapabilities`**
- **`GET /v1/catalog`** — no args, no body → **`CatalogResponse`**
- **`GET /v1/indicators`** — no args, no body → **`[CatalogIndicatorEntry]`**
- **`GET /v1/indicators/{name}`** — **`{name}`** = catalog dot-path, no body → **`CatalogIndicatorEntry`** (404 `unknown_indicator`)
- **`GET /v1/strategies`** — no args, no body → **`[CatalogStrategyEntry]`**
- **`GET /v1/strategies/{id}`** — **`{id}`** = strategy id, no body → **`CatalogStrategyEntry`** (404 `unknown_strategy`)
- **`POST /v1/indicators/{name}`** — **`{name}`** = catalog path, body **`MachineRequest`** → **`IndicatorEvaluateResponse`** (404 `unknown_indicator`)
- **`POST /v1/indicators/{name}/replay`** — **`{name}`** = catalog path, body **`IndicatorReplayRequest`** (flattened **`MachineRequest`** + optional **`from_index`** / **`to_index`** / **`step`**, or **`replay_from`** / **`replay_to`** as **`YYYY-MM-DD`** UTC inclusive on each bar’s **`close_time`** — when both dates are set they **override** indices; **`indicators`** ignored) → **`IndicatorReplayResponse`**
- **`POST /v1/indicators/replay`** — no path args, body same **`IndicatorReplayRequest`** but **`indicators`** must be **non-empty** `[dot-path, …]` → **`IndicatorReplayResponse`**
- **`POST /v1/strategies/replay`** — no path args, body **`StrategyReplayRequest`** (flattened **`MachineRequest`** + same index **or** **`replay_from`** / **`replay_to`** rules) → **`StrategyReplayResponse`**

**Errors:** **404** `unknown_indicator` / **`unknown_strategy`**; **400** `invalid_request`; **422** malformed JSON.

## Smoke (`curl`)

Needs **`src/historical_data/btcusd_1-min_data.csv`** (or **`BTCUSD_1M_CSV`**) for bundled POSTs.

```bash
curl -sS http://127.0.0.1:8080/health
curl -sS -X POST 'http://127.0.0.1:8080/v1/indicators/ema_fast' \
  -H 'Content-Type: application/json' \
  -d '{"bar_interval":"1m","bundled_btcusd_1m":{"from":"2012-01-01","to":"2012-01-02"}}'
```

## Replay (minimal Python)

Replay window is either **`replay_from` / `replay_to`** (UTC `YYYY-MM-DD` on each bar’s **`close_time`**) **or** **`from_index` / `to_index`** (integer positions in the series the server built — **not** the same strings as **`bundled_btcusd_1m.from` / `to`**).

**How many bars?** Each element in **`candles`** (or each minute row from bundled CSV) is **one** closed bar. **`bar_interval`** (e.g. **`"15m"`**) describes that row’s timeframe; it does **not** turn one day into “15 bars” by magic — **the number of rows you loaded** (or the slice from **`bundled_btcusd_1m`** / dates) **is** the bar count. **`replay_from` / `replay_to`** pick which of those rows fall in the UTC day window.

**`step`**: advance **N** bars between emitted replay points (**`1`** = every bar in the window). If the window would emit more than **50 000** points at your requested **`step`**, the server **raises `step` automatically** (see `tracing` **warn** logs) instead of failing.

```python
import json, os, urllib.request as u
ENGINE = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
body = {
    "bar_interval": "1m",
    "bundled_btcusd_1m": {"from": "2012-01-01", "to": "2012-01-03"},
    "replay_from": "2012-01-02",
    "replay_to": "2012-01-02",
    "step": 1,
}
req = u.Request(
    f"{ENGINE}/v1/indicators/ema_fast/replay",
    data=json.dumps(body).encode(),
    headers={"Content-Type": "application/json; charset=utf-8"},
    method="POST",
)
print(u.urlopen(req, timeout=120).read().decode())
```

**`bundled_btcusd_1m.from` / `to`** = UTC calendar slice of the CSV; **`replay_from` / `replay_to`** = replay window on that slice’s bar **`close_time`**s. Multi-indicator: **`POST …/v1/indicators/replay`** with **`"indicators": ["ema_fast", "atr"]`** plus the same replay fields.

**Bundled CSV — load the whole file (`all: true`)**, then replay by **bar indices** (no **`replay_*`** dates here — the CSV slice is already “all rows”). Use **`from_index`…`to_index`** to bound work; **`step`** may be auto-raised if the span is huge:

```python
import json, os, urllib.request as u
ENGINE = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
body = {
    "bar_interval": "1m",
    "bundled_btcusd_1m": {"all": True},
    "from_index": 0,
    "to_index": 2000,
    "step": 1,  # every bar from index 0 through 2000 inclusive
}
req = u.Request(
    f"{ENGINE}/v1/indicators/ema_fast/replay",
    data=json.dumps(body).encode(),
    headers={"Content-Type": "application/json; charset=utf-8"},
    method="POST",
)
print(u.urlopen(req, timeout=600).read().decode())
```

Do **not** set **`from`**/**`to`** on **`bundled_btcusd_1m`** when **`all`** is true.
