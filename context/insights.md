# insights

- **2026-04-18:** Vol baseline default 960 bars → `paper_bot` / HTTP payloads need enough `15m` history unless `VOL_BASELINE_LOOKBACK_BARS` is lowered.
- **2026-04-18:** Multi-strategy HTTP backtests: default R:R implies ~40% breakeven win rate; observed baseline ~40.1% ⇒ no meaningful edge. Relaxing tunable gates increases trades but dilutes sample (worse PF / avg R).
