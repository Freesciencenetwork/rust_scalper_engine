# schema

- **HTTP POST bodies** (indicator last-bar, indicator replay, strategy replay): serde-flattens into one JSON root — see **`MachineRequest`** + replay fields in [`src/machine.rs`](../src/machine.rs) (`IndicatorReplayRequest`, `StrategyReplayRequest` flatten `machine: MachineRequest`).
- **Bar source (exactly one):** non-empty **`candles`** (alias **`candles_15m`**) **or** **`bundled_btcusd_1m`** (`from` / `to` / `all` + CSV from **`BTCUSD_1M_CSV`** or default path) **or** **`synthetic_series`** (demo OHLCV). Optional **`bar_interval`**, **`runtime_state`**, **`config_overrides`**, etc.
- **Replay-only (same JSON root):** **`from_index`**, **`to_index`**, **`step`**; plus **`indicators`** array for **`POST /v1/indicators/replay`** only.
- **Candle (JSON):** `close_time` (ms since epoch), `open`, `high`, `low`, `close`, `volume`; optional `buy_volume`, `sell_volume`, `delta`. Oldest → newest.
- **Bundled CSV:** header with `Timestamp` (unix **seconds**, float ok) + `Open`,`High`,`Low`,`Close`,`Volume` — see [`src/historical_data/mod.rs`](../src/historical_data/mod.rs). Optional: Python `kagglehub` + unzip — README.
- **Binance REST JSON:** **`fetch_max_btcusdt_1m`** → root **`candles`** + **`bar_interval`**; **`binance-fetch`** may still emit **`candles_15m`** (legacy JSON key, not TF) — serde alias on **`MachineRequest`** — README.
