# state

- **In repo:** Rust `paper_bot` demo (`cargo run --bin paper_bot`); HTTP `server` for `POST /v1/evaluate`.
- **Last change:** Removed Python-based HTTP demo and historical walk-forward docs from this repo.
- **2026-04-18 — Backtest table follow-up:** Baseline ~40% win + PF ~1.14 matches breakeven math for 2×ATR stop / 3×ATR target (~40% needed). Engine behavior aligns with spec; edge is ~zero before costs. `no_runway` gates clearance with `stop_atr_multiple` not `target_atr_multiple` (possible design bug). See `memory.txt`.
