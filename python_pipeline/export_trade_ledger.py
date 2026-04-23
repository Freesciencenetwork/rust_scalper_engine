"""
export_trade_ledger.py — Fetch a Rust backtest ledger and join it to normalized features.

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
import os
import sys
import urllib.error
import urllib.request

import pandas as pd

import config
from normalize_features import feature_columns

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)


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
    if args.from_date and args.to_date:
        from_day = dt.date.fromisoformat(args.from_date)
        warmup_start = from_day - dt.timedelta(days=args.warmup_days)
        body["bundled_btcusd_1m"] = {
            "from": warmup_start.isoformat(),
            "to": args.to_date,
        }
        body["replay_from"] = args.from_date
        body["replay_to"] = args.to_date
    else:
        body["bundled_btcusd_1m"] = {"all": True}
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


def main():
    args = parse_args()
    body = build_request_body(args)
    logger.info("Requesting backtest ledger for strategy=%s", args.strategy)
    response = fetch_backtest(args.engine_url, args.strategy, body)

    trades = response.get("trades", [])
    if not trades:
        raise SystemExit("backtest returned zero resolved trades")

    trades_df = pd.DataFrame(trades)
    trades_df.rename(columns={"signal_close_time": "signal_close_time_ms"}, inplace=True)
    trades_df["strategy_id"] = response.get("strategy_id", args.strategy)
    for key, value in body["execution"].items():
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
                "strategy_id": response.get("strategy_id", args.strategy),
                "summary": response.get("summary", {}),
                "feature_count": len(feat_cols),
                "output_rows": len(joined),
            },
            fh,
            indent=2,
        )
    logger.info("Wrote summary -> %s", summary_path)


if __name__ == "__main__":
    main()
