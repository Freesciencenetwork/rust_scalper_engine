"""
build_trade_ledger.py — Fetch a Rust backtest ledger and join it to normalized features.

Writes one row per resolved trade:
  signal_close_time_ms + feature columns + trade outcome columns

Warm-up protection:
  rows with NaN in any feature column are dropped after the feature join.

When exporting a 15m strategy such as `default`, the script asks Rust to load the
bundled 1m CSV and resample it server-side before backtesting. This keeps the HTTP
payload small while preserving the strategy timeframe.
"""

import argparse
import datetime as dt
import json
import logging
import math
import os
import sys
import urllib.error
import urllib.request

import pandas as pd

from python_pipeline.features.build_feature_cache import feature_columns
from python_pipeline.shared import pipeline_config as config

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)
CHUNK_MONTHS = 6


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--strategy", default="default")
    parser.add_argument(
        "--engine-url",
        default=os.environ.get("ENGINE_URL", "http://127.0.0.1:8080"),
    )
    parser.add_argument(
        "--features",
        default=os.path.join(config.DATA_DIR, "features_normalized.parquet"),
    )
    parser.add_argument(
        "--bar-interval",
        default="15m",
        help="Server-side resample interval for bundled 1m BTC data (default: 15m)",
    )
    parser.add_argument("--from-date", default=None, help="UTC YYYY-MM-DD")
    parser.add_argument("--to-date", default=None, help="UTC YYYY-MM-DD")
    parser.add_argument(
        "--warmup-days",
        type=int,
        default=14,
        help="Warm-up days loaded before --from-date when a date window is used",
    )
    parser.add_argument("--entry-fee-bps", type=float, default=10.0)
    parser.add_argument("--exit-fee-bps", type=float, default=10.0)
    parser.add_argument("--entry-slippage-bps", type=float, default=2.0)
    parser.add_argument("--exit-slippage-bps", type=float, default=2.0)
    parser.add_argument("--stop-extra-slippage-bps", type=float, default=3.0)
    parser.add_argument("--max-hold-bars", type=int, default=20)
    parser.add_argument("--output", default=None)
    return parser.parse_args()


def build_request_body(args) -> dict:
    if bool(args.from_date) != bool(args.to_date):
        raise ValueError("set both --from-date and --to-date, or neither")
    if args.warmup_days < 0:
        raise ValueError("--warmup-days must be >= 0")

    body = {
        "bar_interval": args.bar_interval,
        "bundled_resample_interval": args.bar_interval,
        "execution": {
            "entry_fee_bps": args.entry_fee_bps,
            "exit_fee_bps": args.exit_fee_bps,
            "entry_slippage_bps": args.entry_slippage_bps,
            "exit_slippage_bps": args.exit_slippage_bps,
            "stop_extra_slippage_bps": args.stop_extra_slippage_bps,
            "max_hold_bars": args.max_hold_bars,
        },
    }
    if not (args.from_date and args.to_date):
        body["bundled_btcusd_1m"] = {"all": True}
    return body


def interval_label_to_minutes(label: str) -> int:
    text = label.strip().lower()
    if not text:
        raise ValueError("bar interval must not be empty")
    if text.endswith("m"):
        return int(text[:-1])
    if text.endswith("h"):
        return int(text[:-1]) * 60
    if text.endswith("d"):
        return int(text[:-1]) * 24 * 60
    if text.endswith("w"):
        return int(text[:-1]) * 7 * 24 * 60
    raise ValueError(f"unsupported bar interval {label!r}; use labels like 15m, 1h")


def date_range_chunks(start: dt.date, end: dt.date, months: int):
    cur = start
    while cur <= end:
        chunk_end = dt.date(
            cur.year + (cur.month - 1 + months) // 12,
            (cur.month - 1 + months) % 12 + 1,
            1,
        ) - dt.timedelta(days=1)
        chunk_end = min(chunk_end, end)
        yield cur, chunk_end
        cur = chunk_end + dt.timedelta(days=1)


def forward_pad_days(args) -> int:
    interval_minutes = interval_label_to_minutes(args.bar_interval)
    hold_minutes = args.max_hold_bars * interval_minutes
    return max(1, math.ceil(hold_minutes / (24 * 60)) + 1)


def day_start_ms(day: dt.date) -> int:
    return int(dt.datetime(day.year, day.month, day.day, tzinfo=dt.timezone.utc).timestamp() * 1000)


def day_end_ms(day: dt.date) -> int:
    return day_start_ms(day + dt.timedelta(days=1)) - 1


def build_chunk_request_body(args, chunk_start: dt.date, chunk_end: dt.date) -> dict:
    body = build_request_body(args)
    warmup_start = chunk_start - dt.timedelta(days=args.warmup_days)
    padded_end = chunk_end + dt.timedelta(days=forward_pad_days(args))
    body["bundled_btcusd_1m"] = {
        "from": warmup_start.isoformat(),
        "to": padded_end.isoformat(),
    }
    body["replay_from"] = chunk_start.isoformat()
    body["replay_to"] = padded_end.isoformat()
    return body


def fetch_backtest(engine_url: str, strategy: str, body: dict) -> dict:
    engine = engine_url.rstrip("/")
    url = f"{engine}/v1/strategies/{strategy}/backtest"
    payload = json.dumps(body).encode()
    req = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=600) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode(errors="replace")
        raise RuntimeError(f"HTTP {exc.code} from {url}: {detail}") from exc


def join_features(trades_df: pd.DataFrame, features_path: str) -> tuple[pd.DataFrame, list]:
    logger.info("Loading normalized features from %s", features_path)
    feats = pd.read_parquet(features_path).sort_values("timestamp_ms").reset_index(drop=True)
    feat_cols = feature_columns(feats)

    trades = trades_df.sort_values("signal_close_time_ms").reset_index(drop=True)
    joined = pd.merge(
        trades,
        feats,
        left_on="signal_close_time_ms",
        right_on="timestamp_ms",
        how="left",
    )
    before = len(joined)
    joined.dropna(subset=feat_cols, inplace=True)
    joined.reset_index(drop=True, inplace=True)
    logger.info(
        "Feature join kept %d / %d trade rows after exact-timestamp join and NaN warm-up filtering",
        len(joined),
        before,
    )
    return joined, feat_cols


def fetch_trade_ledger(args) -> tuple[pd.DataFrame, str]:
    if not (args.from_date and args.to_date):
        response = fetch_backtest(args.engine_url, args.strategy, build_request_body(args))
        trades_df = pd.DataFrame(response.get("trades", []))
        strategy_id = response.get("strategy_id", args.strategy)
        if trades_df.empty:
            return trades_df, strategy_id
        trades_df.rename(columns={"signal_close_time": "signal_close_time_ms"}, inplace=True)
        return trades_df, strategy_id

    start = dt.date.fromisoformat(args.from_date)
    end = dt.date.fromisoformat(args.to_date)
    frames = []
    strategy_id = args.strategy

    for i, (chunk_start, chunk_end) in enumerate(date_range_chunks(start, end, CHUNK_MONTHS), start=1):
        logger.info(
            "Backtest chunk %d: %s -> %s",
            i,
            chunk_start.isoformat(),
            chunk_end.isoformat(),
        )
        response = fetch_backtest(
            args.engine_url,
            args.strategy,
            build_chunk_request_body(args, chunk_start, chunk_end),
        )
        strategy_id = response.get("strategy_id", strategy_id)
        trades_df = pd.DataFrame(response.get("trades", []))
        if trades_df.empty:
            continue
        trades_df.rename(columns={"signal_close_time": "signal_close_time_ms"}, inplace=True)
        trades_df = trades_df[
            trades_df["signal_close_time_ms"].between(
                day_start_ms(chunk_start),
                day_end_ms(chunk_end),
            )
        ].reset_index(drop=True)
        if not trades_df.empty:
            frames.append(trades_df)

    if not frames:
        return pd.DataFrame(), strategy_id

    merged = pd.concat(frames, ignore_index=True)
    merged.drop_duplicates(subset=["signal_close_time_ms"], keep="first", inplace=True)
    merged.sort_values("signal_close_time_ms", inplace=True)
    merged.reset_index(drop=True, inplace=True)
    return merged, strategy_id


def main():
    args = parse_args()
    logger.info("Requesting backtest ledger for strategy=%s", args.strategy)
    trades_df, strategy_id = fetch_trade_ledger(args)
    if trades_df.empty:
        raise SystemExit("backtest returned zero resolved trades")

    trades_df["strategy_id"] = strategy_id
    for key, value in build_request_body(args)["execution"].items():
        trades_df[key] = value

    joined, feat_cols = join_features(trades_df, args.features)
    if joined.empty:
        raise SystemExit("all trade rows were dropped after feature join / warm-up filtering")

    output_path = args.output or os.path.join(
        config.DATA_DIR, f"trade_ledger_{args.strategy}.parquet"
    )
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    joined.to_parquet(output_path, index=False)
    logger.info("Wrote %d joined trade rows -> %s", len(joined), output_path)

    summary_path = os.path.splitext(output_path)[0] + ".summary.json"
    with open(summary_path, "w") as fh:
        json.dump(
            {
                "strategy_id": strategy_id,
                "feature_count": len(feat_cols),
                "trade_rows": len(trades_df),
                "output_rows": len(joined),
            },
            fh,
            indent=2,
        )
    logger.info("Wrote summary -> %s", summary_path)


if __name__ == "__main__":
    main()
