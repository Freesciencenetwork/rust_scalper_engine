#!/usr/bin/env python3
"""
Example: replay **three** catalog indicators over candles restricted to **2010-01-01 … 2012-12-31** (UTC).

Alternatively use **`bundled_btcusd_1m`** + **`from`/`to`** in JSON (no local candle file) — see README.

Flow
  1. Build or load `candles` (each row must have `close_time` in **milliseconds**).
  2. **Filter** rows by `close_time` — the engine has no `start_year` parameter; your timeframe is the
     array you POST.
  3. `POST /v1/indicators/replay` with a non-empty `indicators` list (exact dot-paths from `GET /v1/catalog`).

Requires: running server (`cargo run`), and a large enough POST body (e.g. `src/historical_data/request.json`
from `cargo run --release --bin fetch_max_btcusdt_1m` or a one-page `binance-fetch`) or embedded candles.

Usage:
  python3 examples/indicators_replay_date_range.py src/historical_data/request.json
  ENGINE_URL=http://127.0.0.1:8080 python3 examples/indicators_replay_date_range.py src/historical_data/request.json
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone

# UTC boundaries (inclusive by close_time; adjust if you want exclusive end).
START_2010_MS = int(datetime(2010, 1, 1, tzinfo=timezone.utc).timestamp() * 1000)
END_2012_MS = int(datetime(2012, 12, 31, 23, 59, 59, 999000, tzinfo=timezone.utc).timestamp() * 1000)


def filter_candles_2010_2012(candles: list[dict]) -> list[dict]:
    return [c for c in candles if START_2010_MS <= int(c["close_time"]) <= END_2012_MS]


def post_json(url: str, path: str, payload: dict) -> tuple[int, str]:
    body = json.dumps(payload).encode("utf-8")
    headers = {"Content-Type": "application/json; charset=utf-8"}
    req = urllib.request.Request(url.rstrip("/") + path, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=300) as resp:
            return resp.status, resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8")


def main() -> int:
    if len(sys.argv) < 2:
        print(
            "Usage: python3 examples/indicators_replay_date_range.py <path/to/request.json>",
            file=sys.stderr,
        )
        return 2

    path = sys.argv[1]
    with open(path, encoding="utf-8") as f:
        base = json.load(f)

    candles = base.get("candles") or base.get("candles_15m")
    if not candles:
        print("JSON must contain candles or candles_15m array.", file=sys.stderr)
        return 1

    filtered = filter_candles_2010_2012(candles)
    if not filtered:
        print(
            "No candles in 2010-2012 range; widen your fetch or check close_time units (ms).",
            file=sys.stderr,
        )
        return 1

    # Minimal body: `candles` + replay fields. Other `MachineRequest` keys default in Rust
    # (empty macro list, default runtime_state, no sizing / overrides) — add only if you need them.
    payload = {
        "candles": filtered,
        "bar_interval": base.get("bar_interval", "15m"),
        "indicators": [
            "ema_fast",
            "atr",
            "indicator_snapshot.momentum.rsi_14",
        ],
        "from_index": 0,
        "to_index": len(filtered) - 1,
        "step": 1,
    }

    base_url = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
    print(
        f"POST {base_url}/v1/indicators/replay  "
        f"(candles_in_range={len(filtered)} of {len(candles)} total)",
        file=sys.stderr,
    )
    status, text = post_json(base_url, "/v1/indicators/replay", payload)
    print(f"HTTP {status}", file=sys.stderr)
    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        print(text)
        return 1

    steps = data.get("steps") or []
    print(json.dumps({"step_count": len(steps), "first": steps[:1], "last": steps[-1:]}, indent=2))
    return 0 if status == 200 else 1


if __name__ == "__main__":
    raise SystemExit(main())
