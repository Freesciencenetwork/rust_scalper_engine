# insights

- **2026-04-18:** Stochastic `%D`: `k_hist[..].iter().flatten()` matches prior `if let Some` loop because `cnt == d_period` still rejects windows with any `None` (flatten shortens count).
- **2026-04-19:** Full-indicator integration test uses `VwapAnchorMode::RollingBars` + `vwap_rolling_bars: Some(96)` so VWAP bands populate without multi-day `UtcDay` history; candles start `2026-01-05` UTC with 15+15*i minutes so `aggregate_15m_to_1h` yields 1h bars.
- **2026-04-18:** Vol baseline default 960 bars → `paper_bot` / HTTP payloads need enough `15m` history unless `VOL_BASELINE_LOOKBACK_BARS` is lowered.
- **2026-04-18:** Multi-strategy HTTP backtests: default R:R implies ~40% breakeven win rate; observed baseline ~40.1% ⇒ no meaningful edge. Relaxing tunable gates increases trades but dilutes sample (worse PF / avg R).
