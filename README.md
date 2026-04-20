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
- **`POST /v1/indicators/{name}/replay`** — **`{name}`** = catalog path, body **`IndicatorReplayRequest`** (flattened **`MachineRequest`** + optional **`replay_from`** / **`replay_to`** UTC **`YYYY-MM-DD`** on each bar’s **`close_time`**, or other window fields in **`src/machine.rs`**; **`indicators`** ignored) → **`IndicatorReplayResponse`**
- **`POST /v1/indicators/replay`** — no path args, body same **`IndicatorReplayRequest`** but **`indicators`** must be **non-empty** `[dot-path, …]` → **`IndicatorReplayResponse`**
- **`POST /v1/strategies/replay`** — no path args, body **`StrategyReplayRequest`** (flattened **`MachineRequest`** + same replay window fields as indicator replay) → **`StrategyReplayResponse`**

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

Smallest useful body: **`bundled_btcusd_1m.from` / `to`** as UTC **`YYYY-MM-DD`** (inclusive calendar days on the CSV). With **no** **`replay_from` / `replay_to`**, replay walks **every** bar in that loaded slice (**`0` … `len-1`**).

**Optional:** **`replay_from` / `replay_to`** — same **`YYYY-MM-DD`** form, filter by each bar’s **`close_time`** when the CSV slice is **wider** than the walk you want (see **`src/machine.rs`**).

**How many bars?** One CSV row = one **1m** bar. Row count = loaded slice unless you add **`replay_*`**.

```python
import json, os, urllib.request as u
ENGINE = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
body = {
    "bar_interval": "1m",
    "bundled_btcusd_1m": {"from": "2012-01-02", "to": "2012-01-02"},
}
req = u.Request(
    f"{ENGINE}/v1/indicators/ema_fast/replay",
    data=json.dumps(body).encode(),
    headers={"Content-Type": "application/json; charset=utf-8"},
    method="POST",
)
print(u.urlopen(req, timeout=120).read().decode())
```

Multi-indicator: **`POST …/v1/indicators/replay`** with **`"indicators": ["ema_fast", "atr"]`** plus the same **`bundled_btcusd_1m`** (and optional **`replay_*`**).

**Backtest-style — `all: true` (max bundled load, walk the whole series)**  
Omit **`replay_from` / `replay_to`**: replay runs from the first loaded bar through the last (**`0` … `len-1`**), like a linear backtest over whatever the server just built. Use **`POST /v1/strategies/replay`** with the same body shape to walk the strategy instead of one indicator.

**Reality checks:** **`all: true`** still hits the bundled loader cap (**500 000** 1m rows from the start of the CSV; anything past that is ignored until you raise the cap in code or split requests by **`from` / `to`**). The replay JSON is also bounded (**about 50 000** steps); very long walks are subsampled server-side so the response stays small (**`src/machine.rs`**).

```python
import json, os, urllib.request as u
ENGINE = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
body = {
    "bar_interval": "1m",
    "bundled_btcusd_1m": {"all": True},
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
