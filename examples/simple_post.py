#!/usr/bin/env python3
"""Minimal POST: last-bar indicator value. Server: `cargo run`."""

from __future__ import annotations

import json
import os
import urllib.request

URL = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
PATH = "/v1/indicators/ema_fast"

# One closed bar is enough for some paths; ema_fast needs warmup — send ~100 bars.
def bars(n: int = 100) -> list[dict]:
    ms, step = 1_700_000_000_000, 15 * 60 * 1000
    out = []
    p = 50_000.0
    for i in range(n):
        t = ms + i * step
        o, p = p, p + 10.0
        out.append({"close_time": t, "open": o, "high": p + 5, "low": o - 3, "close": p, "volume": 1.0})
    return out


def main() -> None:
    body = json.dumps({"candles": bars(), "bar_interval": "15m"}).encode()
    req = urllib.request.Request(
        URL + PATH, data=body, method="POST", headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=60) as r:
        print(r.read().decode())


if __name__ == "__main__":
    main()
