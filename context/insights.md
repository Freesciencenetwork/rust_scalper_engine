# insights

- **2026-04-19:** README teaches **candles-first** then **`POST /v1/indicators/{path}`** (evaluate); **`/replay`** + **`bundled`** = alternate one-call pattern (replay body may use **`candles`** instead).
- **2026-04-19:** **`bundled_btcusd_1m.all: true`**: CSV read stops at **500k** rows (**warn**), not error — matches README “max load backtest” on huge files.
- **2026-04-19:** README replay section is **date-first**; **`step`** / index window not shown in examples (still in **`machine.rs`** / server behavior).
- **2026-04-19:** README API = **one line per route** (bullets), not a table.
- **2026-04-19:** **`replay_from`** / **`replay_to`** (`YYYY-MM-DD` UTC) on replay bodies map to bar indices by **`close_time`**; override **`from_index`**/**`to_index`** when both set.
- **2026-04-19:** Full Binance download = **`cargo run --release --bin fetch_max_btcusdt_1m`** (`binance_spot_candles::fetch_klines` loop); **`binance-fetch`** = single ≤1000 page. README **`curl`** uses **`src/historical_data/request.json`**; `.gitignore` **`request.json`** / **`request.trimmed.json`**.
- **2026-04-19:** After dropping optional **`kaggle`** from `Cargo.toml`, run **`cargo generate-lockfile`** (or `cargo build`) so **`cargo test --locked`** succeeds; stale lock still listed `kaggle` until regen.
- **2026-04-19 (perf):** `PreparedDataset::build` dominates per-request CPU/RAM; scale aggregate QPS with **N stateless replicas**. `EVALUATE_MAX_INFLIGHT` optional (unset = hardware/Tokio only; set = per-instance cap). Release LTO + last-bar `pop()` reduce waste.
- **2026-04-19 (supply-chain):** `cargo audit` on current `Cargo.lock` reported **zero** in-scope RUSTSEC/CVE matches; correlate any future advisory to locked versions (not semver ranges in `Cargo.toml` alone).
- **2026-04-18:** Stochastic `%D`: `k_hist[..].iter().flatten()` matches prior `if let Some` loop because `cnt == d_period` still rejects windows with any `None` (flatten shortens count).
- **2026-04-19:** Full-indicator integration test uses `VwapAnchorMode::RollingBars` + `vwap_rolling_bars: Some(96)` so VWAP bands populate without multi-day `UtcDay` history; candles start `2026-01-05` UTC with 15+15*i minutes so `aggregate_15m_to_1h` yields 1h bars.
- **2026-04-18:** Vol baseline default 960 bars → `paper_bot` / HTTP payloads need enough `15m` history unless `VOL_BASELINE_LOOKBACK_BARS` is lowered.
- **2026-04-18:** Multi-strategy HTTP backtests: default R:R implies ~40% breakeven win rate; observed baseline ~40.1% ⇒ no meaningful edge. Relaxing tunable gates increases trades but dilutes sample (worse PF / avg R).
