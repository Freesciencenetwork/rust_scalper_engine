#!/usr/bin/env python3
"""POST last-bar indicator using bundled repo CSV (date range or all)."""

from __future__ import annotations

import json
import os
from pathlib import Path
import urllib.request

URL = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
PATH = "/v1/indicators/ema_fast"

REPO = Path(__file__).resolve().parents[1]
# Prefer the full file under src/historical_data/ when present; else tiny fixture (CI / fresh clone).
if "BTCUSD_1M_CSV" not in os.environ:
    real = REPO / "src" / "historical_data" / "btcusd_1-min_data.csv"
    os.environ["BTCUSD_1M_CSV"] = str(
        real if real.exists() else REPO / "tests" / "fixtures" / "btcusd_1m_tiny.csv"
    )

# UTC calendar days, inclusive `to`. Omit `from` or `to` to read from file start / through file end.
# For the whole file: "bundled_btcusd_1m": { "all": true }  (subject to server row cap)
PAYLOAD = {
    "bar_interval": "1m",
    "bundled_btcusd_1m": {
        "from": "2012-01-01",
        "to": "2012-01-03",
    },
}


def main() -> None:
    body = json.dumps(PAYLOAD).encode()
    req = urllib.request.Request(
        URL + PATH, data=body, method="POST", headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        print(r.read().decode())


if __name__ == "__main__":
    main()
