#!/usr/bin/env python3
"""
POST indicator **replay** with an explicit JSON body: choose bar source, replay window, indicator(s),
then print the request and the HTTP response.

  VOL_BASELINE_LOOKBACK_BARS=96 cargo run
  python3 examples/replay_request_file.py
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
ENGINE = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")

# ---------------------------------------------------------------------------
# Parameters → one `MachineRequest`-shaped JSON (same keys the HTTP server reads)
# ---------------------------------------------------------------------------

# Bar source (pick exactly one shape in the body):
#   "file"     — load `candles` (+ `bar_interval`) from JSON on disk
#   "bundled"  — server reads CSV; use `bundled_btcusd_1m` with either `from`+`to` OR `all: true`
DATA_SOURCE = "bundled"
CANDLE_JSON = REPO / "src/historical_data/request.json"

# When DATA_SOURCE == "bundled" — UTC calendar days `YYYY-MM-DD`, or whole file:
BUNDLED_FROM_TO = {"from": "2012-01-01", "to": "2012-01-03"}
BUNDLED_ALL = {"all": True}
USE_BUNDLED_ALL = False  # if True, body uses BUNDLED_ALL; else BUNDLED_FROM_TO

BAR_INTERVAL = "1m"

# Replay window — pick one style (server: see README / `IndicatorReplayRequest`):
#   A) UTC calendar days on each bar's `close_time` (`YYYY-MM-DD`), inclusive:
USE_REPLAY_DAYS = True
REPLAY_FROM = "2012-01-02"
REPLAY_TO = "2012-01-02"
#   B) Bar indices (ignored if USE_REPLAY_DAYS). Omit TO_INDEX in JSON → through last bar.
FROM_INDEX = 0
TO_INDEX = 500  # or None
STEP = 1

# Indicators:
#   "single" → POST /v1/indicators/{INDICATOR}/replay  (one catalog leaf, e.g. ema_fast)
#   "multi"  → POST /v1/indicators/replay             (non-empty `indicators` list, dot-paths)
REPLAY_MODE = "single"
INDICATOR = "ema_fast"
INDICATORS = ["ema_fast", "atr"]

MAX_PRINT = 20_000


def post(path: str, body: dict) -> tuple[int, str]:
    raw = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(
        ENGINE + path,
        data=raw,
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=600) as resp:
            return resp.status, resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8")


def build_body() -> tuple[dict, str, str]:
    if DATA_SOURCE == "file":
        with CANDLE_JSON.open(encoding="utf-8") as f:
            saved = json.load(f)
        candles = saved.get("candles") or saved.get("candles_15m")
        if not candles:
            raise SystemExit(f"{CANDLE_JSON}: need non-empty `candles` (or legacy `candles_15m`)")
        body: dict = {
            "candles": candles,
            "bar_interval": saved.get("bar_interval") or BAR_INTERVAL,
        }
        try:
            path_hint = str(CANDLE_JSON.relative_to(REPO))
        except ValueError:
            path_hint = str(CANDLE_JSON)
    elif DATA_SOURCE == "bundled":
        bundled = dict(BUNDLED_ALL if USE_BUNDLED_ALL else BUNDLED_FROM_TO)
        body = {"bar_interval": BAR_INTERVAL, "bundled_btcusd_1m": bundled}
        path_hint = "bundled_btcusd_1m (server reads CSV)"
    else:
        raise SystemExit('DATA_SOURCE must be "file" or "bundled"')

    body["step"] = STEP
    if USE_REPLAY_DAYS:
        body["replay_from"] = REPLAY_FROM
        body["replay_to"] = REPLAY_TO
    else:
        body["from_index"] = FROM_INDEX
        if TO_INDEX is not None:
            body["to_index"] = TO_INDEX

    if REPLAY_MODE == "multi":
        body["indicators"] = list(INDICATORS)
        path = "/v1/indicators/replay"
    elif REPLAY_MODE == "single":
        path = f"/v1/indicators/{INDICATOR.lstrip('/')}/replay"
    else:
        raise SystemExit('REPLAY_MODE must be "single" or "multi"')

    return body, path, path_hint


def main() -> int:
    try:
        body, path, path_hint = build_body()
    except SystemExit as e:
        print(e, file=sys.stderr)
        return 2

    print("=== REQUEST (JSON body sent to server) ===\n")
    s = json.dumps(body, indent=2)
    if len(s) > MAX_PRINT:
        print(s[:MAX_PRINT] + "\n… truncated for display …\n")
    else:
        print(s + "\n")

    print(f"=== POST {ENGINE}{path}  ({path_hint}) ===\n", file=sys.stderr)

    status, text = post(path, body)
    print(f"=== RESPONSE (HTTP {status}) ===\n")
    try:
        parsed = json.loads(text)
        out = json.dumps(parsed, indent=2)
    except json.JSONDecodeError:
        parsed = None
        out = text
    print(out[:MAX_PRINT] + ("…\n" if len(out) > MAX_PRINT else "\n"))

    if status != 200:
        return 1
    steps = (parsed or {}).get("steps") or []
    print(f"(replay steps: {len(steps)})", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
