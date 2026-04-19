# decisions

- **2026-04-18:** Python orchestration / JSONL walk-forward harness not tracked in this repo; use external HTTP clients or the Rust library.
- **2026-04-19:** Multi-bar **indicator** testing via **`POST /v1/indicators/{path}/replay`** and **`POST /v1/indicators/replay`** (flattened `MachineRequest` + optional `from_index` / `to_index` / `step`; multi route requires non-empty **`indicators`**). **Strategy** linear walk via **`POST /v1/strategies/replay`** with the same flattened body + optional index window. **50k** emitted steps cap per replay request for abuse safety.
- **2026-04-19:** **`GET /v1/catalog`** is the discovery source of truth for indicator dot-paths; execution is per-indicator POSTs and replays above (no separate `evaluate_multi` HTTP route in current `server.rs`).
