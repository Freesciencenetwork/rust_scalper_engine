#!/usr/bin/env python3
"""
Talk to the rust_scalper_engine HTTP API from Python (stdlib only). No API keys — the server
does not perform HTTP authentication (protect the listener with your own network / proxy if needed).

Indicator endpoints
  • GET /v1/catalog — strategy ids + indicator dot paths.
  • GET /v1/indicators — list entries.
  • POST /v1/indicators/{name} — last-bar value for one catalog path.
  • POST /v1/indicators/{name}/replay — one indicator over a bar window.
  • POST /v1/indicators/replay — multiple indicators (non-empty `indicators` list in body).

Strategy linear replay
  • POST /v1/strategies/replay — JSON steps with per-bar SignalDecision (optional from_index, to_index, step).

Run the server (shorter warmup in dev):
  VOL_BASELINE_LOOKBACK_BARS=96 cargo run

Then:
  python3 examples/engine_http_client.py
  python3 examples/engine_http_client.py --catalog
  python3 examples/engine_http_client.py --replay
  python3 examples/engine_http_client.py --strategy-replay
  ENGINE_URL=http://127.0.0.1:8080 python3 examples/engine_http_client.py
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request


def synthetic_candles(count: int, start_close_ms: int) -> list[dict]:
    """Boring uptrend bars so the request is structurally valid (not trading advice)."""
    out: list[dict] = []
    step_ms = 15 * 60 * 1000
    price = 80_000.0
    for i in range(count):
        t = start_close_ms + i * step_ms
        o = price
        price += 12.0
        c = price
        h = c + 40.0
        lo = o - 25.0
        vol = 100.0 + float(i)
        buy_v = vol * 0.62
        sell_v = vol - buy_v
        out.append(
            {
                "close_time": t,
                "open": o,
                "high": h,
                "low": lo,
                "close": c,
                "volume": vol,
                "buy_volume": buy_v,
                "sell_volume": sell_v,
                "delta": None,
            }
        )
    return out


def machine_request(candle_count: int) -> dict:
    start_ms = 1_744_676_100_000
    return {
        "candles": synthetic_candles(candle_count, start_ms),
        "bar_interval": "15m",
    }


def get_json(url: str, path: str) -> tuple[int, str]:
    req = urllib.request.Request(url.rstrip("/") + path, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return resp.status, resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8")


def post_json(url: str, path: str, payload: dict) -> tuple[int, str]:
    body = json.dumps(payload).encode("utf-8")
    headers = {"Content-Type": "application/json; charset=utf-8"}
    req = urllib.request.Request(url.rstrip("/") + path, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            return resp.status, resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8")


def main() -> int:
    p = argparse.ArgumentParser(description="Call the engine HTTP API (indicator or strategy replay JSON).")
    p.add_argument(
        "--catalog",
        action="store_true",
        help="GET /v1/catalog.",
    )
    p.add_argument(
        "--replay",
        action="store_true",
        help="POST /v1/indicators/ema_fast/replay with from_index/to_index/step.",
    )
    p.add_argument(
        "--strategy-replay",
        action="store_true",
        help="POST /v1/strategies/replay (linear strategy decisions).",
    )
    p.add_argument("--from-index", type=int, default=80)
    p.add_argument("--to-index", type=int, default=95)
    p.add_argument("--step", type=int, default=1)
    p.add_argument("--candles", type=int, default=96, help="How many 15m bars (>= ~96 for dev server).")
    args = p.parse_args()

    base = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")

    if args.catalog + args.replay + args.strategy_replay > 1:
        print("Choose at most one of --catalog, --replay, --strategy-replay.", file=sys.stderr)
        return 2

    if args.catalog:
        print(f"GET {base}/v1/catalog", file=sys.stderr)
        status, text = get_json(base, "/v1/catalog")
        print(f"HTTP {status}", file=sys.stderr)
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            print(text)
            return 1
        print(json.dumps(data, indent=2)[:12_000])
        return 0 if status == 200 else 1

    payload = machine_request(args.candles)
    if args.replay:
        path = "/v1/indicators/ema_fast/replay"
        payload = {
            **payload,
            "from_index": args.from_index,
            "to_index": args.to_index,
            "step": args.step,
        }
    elif args.strategy_replay:
        path = "/v1/strategies/replay"
        payload = {
            **payload,
            "from_index": args.from_index,
            "to_index": args.to_index,
            "step": args.step,
        }
    else:
        path = "/v1/indicators/ema_fast"

    print(f"POST {base}{path}  (candles={args.candles})", file=sys.stderr)
    status, text = post_json(base, path, payload)
    print(f"HTTP {status}", file=sys.stderr)

    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        print(text)
        return 1

    print(json.dumps(data, indent=2)[:12_000])
    if len(json.dumps(data)) > 12_000:
        print("\n… output truncated for terminal …", file=sys.stderr)

    if status != 200:
        return 1

    if args.replay:
        steps = data.get("steps") or []
        print(f"\nSummary: {len(steps)} indicator replay steps", file=sys.stderr)
    elif args.strategy_replay:
        steps = data.get("steps") or []
        sid = data.get("strategy_id")
        print(f"\nSummary: strategy_id={sid!r} steps={len(steps)}", file=sys.stderr)
    else:
        print(
            f"\nSummary: path={data.get('path')!r} computable={data.get('computable')!r}",
            file=sys.stderr,
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
