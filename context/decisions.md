# decisions

- **2026-04-18:** Python orchestration / JSONL walk-forward harness not tracked in this repo; use Rust `paper_bot` or external clients against the HTTP API if needed.
- **2026-04-19:** Multi-bar historical testing via **`POST /v1/evaluate_replay`** (flattened `MachineRequest` + optional `from_index`/`to_index`/`step`) instead of N prefix posts to `/v1/evaluate`; 50k step cap for abuse safety.
- **2026-04-19:** **`GET /v1/catalog`** + **`POST /v1/evaluate_multi`**: discovery vs execution split; multi response filters flattened `PreparedCandle` JSON by path (indicator compute path unchanged — filter is serialization-only).
- **2026-04-19:** **`bar_interval`** + **`candles`** alias: semantic labeling + JSON ergonomics; **`min_bars_required`** is best-effort / config-aware, not a guarantee for every edge case in `prepare.rs`.
