#!/usr/bin/env python3
“””
Talk to the rust_scalper_engine HTTP API from Python (stdlib only).

What you send
  • `candles` — closed OHLCV bars, oldest → newest (any timeframe).
  • Optional: `macro_events`, `runtime_state`, `account_equity`, `symbol_filters`,
    `config_overrides` (e.g. `higher_tf_factor`, `vwap_anchor_mode`).

What you do NOT send
  • You do not send “RSI” or “MACD” values. The Rust engine computes all indicators
    from your OHLCV candles.

Indicator endpoints (HTTP)
  • `GET /v1/catalog` — lists strategy ids + valid indicator dot paths.
  • `GET /v1/indicators` — list all indicator entries.
  • `GET /v1/indicators/{name}` — metadata for one indicator.
  • `POST /v1/indicators/{name}` — compute last-bar value for one indicator.
  • `POST /v1/indicators/{name}/replay` — replay one indicator over a bar window.
  • `POST /v1/indicators/replay` — replay multiple indicators (list in body).

Run the server first (dev-friendly shorter history):
  VOL_BASELINE_LOOKBACK_BARS=96 cargo run --bin server

Then:
  python3 examples/engine_http_client.py
  python3 examples/engine_http_client.py --catalog
  python3 examples/engine_http_client.py --multi
  ENGINE_URL=http://127.0.0.1:8080 EVALUATE_API_KEY=secret python3 examples/engine_http_client.py
“””

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
    # Same millisecond style as repo fixtures (Binance close time convention).
    start_ms = 1_744_676_100_000
    return {
        "candles": synthetic_candles(candle_count, start_ms),
        "bar_interval": "15m",
        "macro_events": [],
        "runtime_state": {
            "realized_net_r_today": 0.0,
            "halt_new_entries_flag": 0,
        },
        "account_equity": 100_000.0,
        "symbol_filters": {"tick_size": 0.1, "lot_step": 0.001},
        # Omit strategy → default. Try e.g. "macd_trend", "rsi_pullback", … if built in.
        "config_overrides": {"strategy_id": "default"},
    }


def get_json(url: str, path: str) -> tuple[int, str]:
    req = urllib.request.Request(url.rstrip("/") + path, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return resp.status, resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8")


def post_json(url: str, path: str, payload: dict, api_key: str | None) -> tuple[int, str]:
    body = json.dumps(payload).encode("utf-8")
    headers = {"Content-Type": "application/json; charset=utf-8"}
    if api_key:
        headers["X-Api-Key"] = api_key
    req = urllib.request.Request(url.rstrip("/") + path, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            return resp.status, resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8")


def main() -> int:
    p = argparse.ArgumentParser(description="POST MachineRequest JSON to the engine HTTP API.")
    p.add_argument(
        "--catalog",
        action="store_true",
        help="GET /v1/catalog (strategy ids + indicator path list). No API key.",
    )
    p.add_argument(
        "--multi",
        action="store_true",
        help="POST /v1/evaluate_multi (see --strategies / --indicators).",
    )
    p.add_argument(
        "--replay",
        action="store_true",
        help="Call /v1/evaluate_replay with from_index/to_index/step on the same payload shape.",
    )
    p.add_argument("--from-index", type=int, default=80)
    p.add_argument("--to-index", type=int, default=95)
    p.add_argument("--step", type=int, default=1)
    p.add_argument("--candles", type=int, default=96, help="How many 15m bars (>= ~96 for dev server).")
    p.add_argument(
        "--strategies",
        default="default,macd_trend",
        help="Comma-separated strategy ids for --multi (must match GET /v1/catalog).",
    )
    p.add_argument(
        "--indicators",
        default="ema_fast_15m,indicator_snapshot.momentum.rsi_14",
        help="Comma-separated indicator paths for --multi (empty string = all paths).",
    )
    args = p.parse_args()

    base = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080").rstrip("/")
    api_key = os.environ.get("EVALUATE_API_KEY", "").strip() or None

    if args.catalog + args.multi + args.replay > 1:
        print("Choose at most one of --catalog, --multi, --replay.", file=sys.stderr)
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
    if args.multi:
        inds = [s.strip() for s in args.indicators.split(",") if s.strip()]
        strats = [s.strip() for s in args.strategies.split(",") if s.strip()]
        path = "/v1/evaluate_multi"
        payload = {**payload, "strategies": strats, "indicators": inds}
    elif args.replay:
        path = "/v1/evaluate_replay"
        payload = {
            **payload,
            "from_index": args.from_index,
            "to_index": args.to_index,
            "step": args.step,
        }
    else:
        path = "/v1/evaluate"

    print(f"POST {base}{path}  (candles={args.candles})", file=sys.stderr)
    status, text = post_json(base, path, payload, api_key)
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

    if args.multi:
        strategies = data.get("strategies") or {}
        inds = data.get("indicators") or {}
        unk = data.get("unknown_indicator_filters") or []
        sample = next(iter(inds.values()), {})
        sample_keys = list(sample.keys()) if isinstance(sample, dict) else []
        print(
            f"\nSummary: strategies={list(strategies.keys())} indicator_keys={len(inds)} "
            f"unknown_filters={unk[:5]} sample_value_fields={sample_keys}",
            file=sys.stderr,
        )
    elif not args.replay:
        action = data.get("action")
        allowed = (data.get("decision") or {}).get("allowed")
        reasons = (data.get("decision") or {}).get("reasons")
        print(f"\nSummary: action={action!r} allowed={allowed!r}", file=sys.stderr)
        if reasons:
            print(f"Reasons ({len(reasons)}): {reasons[:8]}{' …' if len(reasons) > 8 else ''}", file=sys.stderr)
    else:
        steps = data.get("steps") or []
        print(f"\nSummary: {len(steps)} replay steps", file=sys.stderr)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
