"""
collect_indicators.py — Fetch all 132 computable indicator values from the Rust server
and save as a Parquet file for use as ML features.

Strategy
--------
The server has a hard cap of 500,000 bars per request and a 50,000 step
response limit. We paginate through the full 7.5M bar dataset in 6-month
calendar windows, each prefixed with 14 days of warmup history so that
long-period indicators (sma_200, ichimoku, etc.) are fully warm before the
first emitted bar.

Each chunk request:
  bundled_btcusd_1m: {from: warmup_start, to: chunk_end}  ← server reads CSV
  from_index: warmup_bars                                  ← skip warmup in output
  to_index:   <end of loaded slice>

Output: python_pipeline/data/indicators_full.parquet
  - One row per emitted Rust replay bar (1m by default, or a server-side bundled resample)
  - Columns: timestamp_ms, <indicator_path>, ...
  - NaN for bars where indicator hasn't warmed up yet

Usage
  python3 python_pipeline/ingest/collect_indicators.py
  python3 python_pipeline/ingest/collect_indicators.py --server http://localhost:8080 --out python_pipeline/data/indicators_full.parquet
  python3 python_pipeline/ingest/collect_indicators.py --from 2020-01-01 --to 2024-12-31   # partial run
"""

import argparse
import json
import logging
import math
import os
import sys
import time
from datetime import date, timedelta
from typing import Optional
from pathlib import Path

import numpy as np
import pandas as pd
import requests

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)
ROOT_DIR = Path(__file__).resolve().parents[1]
DATA_DIR = ROOT_DIR / "data"
DEFAULT_METADATA_PATH = str(DATA_DIR / "indicators_full.metadata.json")

# ── All indicators computable from OHLCV (no buy/sell volume required) ─────
INDICATORS = [
    "atr",
    "atr_pct",
    "atr_pct_baseline",
    "candle.close",
    "candle.volume",
    "cvd_ema3",
    "cvd_ema3_slope",
    "ema_fast",
    "ema_slow",
    "vol_ratio",
    "vp_poc",
    "vp_vah",
    "vp_val",
    "vwma",
    # Directional
    "indicator_snapshot.directional.adx_14",
    "indicator_snapshot.directional.aroon_down_25",
    "indicator_snapshot.directional.aroon_up_25",
    "indicator_snapshot.directional.di_minus",
    "indicator_snapshot.directional.di_plus",
    "indicator_snapshot.directional.psar",
    "indicator_snapshot.directional.psar_trend_long",
    "indicator_snapshot.directional.vortex_vi_minus_14",
    "indicator_snapshot.directional.vortex_vi_plus_14",
    # Ichimoku
    "indicator_snapshot.ichimoku.chikou_close_shifted",
    "indicator_snapshot.ichimoku.kijun_26",
    "indicator_snapshot.ichimoku.senkou_a_26",
    "indicator_snapshot.ichimoku.senkou_b_52",
    "indicator_snapshot.ichimoku.tenkan_9",
    # Momentum
    "indicator_snapshot.momentum.awesome_oscillator_5_34",
    "indicator_snapshot.momentum.cci_20",
    "indicator_snapshot.momentum.chaikin_oscillator_3_10",
    "indicator_snapshot.momentum.cmo_14",
    "indicator_snapshot.momentum.elder_bear",
    "indicator_snapshot.momentum.elder_bull",
    "indicator_snapshot.momentum.force_index_13",
    "indicator_snapshot.momentum.kst",
    "indicator_snapshot.momentum.kvo_34_55",
    "indicator_snapshot.momentum.kvo_signal_13",
    "indicator_snapshot.momentum.macd_hist",
    "indicator_snapshot.momentum.macd_line",
    "indicator_snapshot.momentum.macd_signal",
    "indicator_snapshot.momentum.mfi_14",
    "indicator_snapshot.momentum.ppo_hist",
    "indicator_snapshot.momentum.ppo_line",
    "indicator_snapshot.momentum.ppo_signal",
    "indicator_snapshot.momentum.pvo_hist",
    "indicator_snapshot.momentum.pvo_line",
    "indicator_snapshot.momentum.pvo_signal",
    "indicator_snapshot.momentum.roc_10",
    "indicator_snapshot.momentum.rsi_14",
    "indicator_snapshot.momentum.stoch_d",
    "indicator_snapshot.momentum.stoch_k",
    "indicator_snapshot.momentum.stoch_rsi_d",
    "indicator_snapshot.momentum.stoch_rsi_k",
    "indicator_snapshot.momentum.trix_15",
    "indicator_snapshot.momentum.trix_signal_9",
    "indicator_snapshot.momentum.tsi_25_13",
    "indicator_snapshot.momentum.ultosc_7_14_28",
    "indicator_snapshot.momentum.williams_r_14",
    # Order flow (session + pattern detection, no buy/sell volume needed)
    "indicator_snapshot.order_flow.in_asia_session",
    "indicator_snapshot.order_flow.in_eu_session",
    "indicator_snapshot.order_flow.in_us_session",
    "indicator_snapshot.order_flow.liquidity_sweep_down",
    "indicator_snapshot.order_flow.liquidity_sweep_up",
    "indicator_snapshot.order_flow.thin_zone",
    "indicator_snapshot.order_flow.vwap_deviation_pct",
    # Candlestick patterns
    "indicator_snapshot.patterns.bear_engulfing",
    "indicator_snapshot.patterns.bull_engulfing",
    "indicator_snapshot.patterns.doji",
    "indicator_snapshot.patterns.hammer",
    "indicator_snapshot.patterns.shooting_star",
    # Trend
    "indicator_snapshot.trend.alma_20",
    "indicator_snapshot.trend.dema_20",
    "indicator_snapshot.trend.ema_20",
    "indicator_snapshot.trend.fama",
    "indicator_snapshot.trend.hist_vol_logrets_20",
    "indicator_snapshot.trend.hull_9",
    "indicator_snapshot.trend.kama_10",
    "indicator_snapshot.trend.lr_slope_20",
    "indicator_snapshot.trend.mama",
    "indicator_snapshot.trend.mcginley_14",
    "indicator_snapshot.trend.price_zscore_20",
    "indicator_snapshot.trend.sma_20",
    "indicator_snapshot.trend.sma_200",
    "indicator_snapshot.trend.sma_50",
    "indicator_snapshot.trend.tema_20",
    "indicator_snapshot.trend.vidya_14",
    "indicator_snapshot.trend.vwap_lower_1sd",
    "indicator_snapshot.trend.vwap_lower_2sd",
    "indicator_snapshot.trend.vwap_session",
    "indicator_snapshot.trend.vwap_upper_1sd",
    "indicator_snapshot.trend.vwap_upper_2sd",
    "indicator_snapshot.trend.wma_20",
    # Volatility
    "indicator_snapshot.volatility.bb_bandwidth_20",
    "indicator_snapshot.volatility.bb_lower_20",
    "indicator_snapshot.volatility.bb_middle_20",
    "indicator_snapshot.volatility.bb_pct_b_20",
    "indicator_snapshot.volatility.bb_upper_20",
    "indicator_snapshot.volatility.chandelier_long_stop_22_3",
    "indicator_snapshot.volatility.chandelier_short_stop_22_3",
    "indicator_snapshot.volatility.donchian_lower_20",
    "indicator_snapshot.volatility.donchian_mid_20",
    "indicator_snapshot.volatility.donchian_upper_20",
    "indicator_snapshot.volatility.keltner_lower_20",
    "indicator_snapshot.volatility.keltner_middle_20",
    "indicator_snapshot.volatility.keltner_upper_20",
    "indicator_snapshot.volatility.mass_index_25",
    "indicator_snapshot.volatility.pivot_classic.pivot_p",
    "indicator_snapshot.volatility.pivot_classic.pivot_r1",
    "indicator_snapshot.volatility.pivot_classic.pivot_r2",
    "indicator_snapshot.volatility.pivot_classic.pivot_r3",
    "indicator_snapshot.volatility.pivot_classic.pivot_s1",
    "indicator_snapshot.volatility.pivot_classic.pivot_s2",
    "indicator_snapshot.volatility.pivot_classic.pivot_s3",
    "indicator_snapshot.volatility.pivot_fib.pivot_p",
    "indicator_snapshot.volatility.pivot_fib.pivot_r1",
    "indicator_snapshot.volatility.pivot_fib.pivot_r2",
    "indicator_snapshot.volatility.pivot_fib.pivot_r3",
    "indicator_snapshot.volatility.pivot_fib.pivot_s1",
    "indicator_snapshot.volatility.pivot_fib.pivot_s2",
    "indicator_snapshot.volatility.pivot_fib.pivot_s3",
    "indicator_snapshot.volatility.supertrend_10_3",
    "indicator_snapshot.volatility.supertrend_long",
    "indicator_snapshot.volatility.ttm_squeeze_momentum",
    "indicator_snapshot.volatility.ttm_squeeze_on",
    # Volume
    "indicator_snapshot.volume.ad_line",
    "indicator_snapshot.volume.cmf_20",
    "indicator_snapshot.volume.nvi",
    "indicator_snapshot.volume.obv",
    "indicator_snapshot.volume.pvi",
    "indicator_snapshot.volume.volume_ema_20",
    "indicator_snapshot.volume.volume_sma_20",
]

WARMUP_DAYS   = 14     # enough for sma_200 (200 min = ~3.3h, but we use 14 days to be safe)
CHUNK_MONTHS  = 6      # size of each calendar chunk
SERVER_TIMEOUT = 300   # seconds per request


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
    raise ValueError(f"unsupported bar interval {label!r}; use labels like 1m, 15m, 1h")


def date_range_chunks(start: date, end: date, months: int):
    """Yield (chunk_start, chunk_end) date pairs with no overlap."""
    cur = start
    while cur <= end:
        chunk_end = date(
            cur.year + (cur.month - 1 + months) // 12,
            (cur.month - 1 + months) % 12 + 1,
            1,
        ) - timedelta(days=1)
        chunk_end = min(chunk_end, end)
        yield cur, chunk_end
        cur = chunk_end + timedelta(days=1)


def fetch_chunk(
    server: str,
    warmup_start: date,
    chunk_end: date,
    warmup_bars: int,
    bar_interval: str,
    bundled_resample_interval: Optional[str],
) -> pd.DataFrame:
    """
    Fetch indicator values for one calendar chunk.

    Requests bars from warmup_start to chunk_end but only emits bars
    starting at from_index=warmup_bars (i.e. skips the warmup prefix).
    """
    url = f"{server}/v1/indicators/replay"
    payload = {
        "bundled_btcusd_1m": {
            "from": warmup_start.isoformat(),
            "to"  : chunk_end.isoformat(),
        },
        "bar_interval": bar_interval,
        "from_index": warmup_bars,
        "indicators": INDICATORS,
    }
    if bundled_resample_interval:
        payload["bundled_resample_interval"] = bundled_resample_interval

    resp = requests.post(url, json=payload, timeout=SERVER_TIMEOUT)
    resp.raise_for_status()
    data = resp.json()
    steps = data.get("steps", [])

    if not steps:
        return pd.DataFrame()

    rows = []
    for step in steps:
        row = {"timestamp_ms": step["close_time"]}
        for ind_path, report in step["indicators"].items():
            row[ind_path] = report["value"]  # None -> NaN after DataFrame construction
        rows.append(row)

    df = pd.DataFrame(rows)
    df["timestamp_ms"] = df["timestamp_ms"].astype("int64")
    return df


def main():
    parser = argparse.ArgumentParser(description="Fetch Rust indicator values for full BTC history")
    parser.add_argument("--server", default="http://localhost:8080")
    parser.add_argument("--out",    default=str(DATA_DIR / "indicators_full.parquet"))
    parser.add_argument("--from",   dest="from_date", default="2012-01-02",
                        help="Start date YYYY-MM-DD (default: 2012-01-02)")
    parser.add_argument("--to",     dest="to_date",   default=None,
                        help="End date YYYY-MM-DD (default: today)")
    parser.add_argument("--bar-interval", default="1m",
                        help="Logical replay interval for the output rows (default: 1m)")
    parser.add_argument("--bundled-resample-interval", default=None,
                        help="Optional server-side resample for bundled 1m CSV input, e.g. 15m")
    parser.add_argument("--metadata-out", default=None,
                        help="Metadata JSON path (default: <out>.metadata.json)")
    args = parser.parse_args()

    start = date.fromisoformat(args.from_date)
    end   = date.fromisoformat(args.to_date) if args.to_date else date.today()
    interval_label = args.bar_interval
    resample_label = args.bundled_resample_interval
    interval_minutes = interval_label_to_minutes(resample_label or interval_label)
    warmup_bars = math.ceil((WARMUP_DAYS * 1440) / interval_minutes)
    metadata_path = args.metadata_out or os.path.splitext(args.out)[0] + ".metadata.json"

    # Health check
    try:
        requests.get(f"{args.server}/health", timeout=5).raise_for_status()
    except Exception as e:
        logger.error("Server not reachable at %s: %s", args.server, e)
        sys.exit(1)

    logger.info(
        "Fetching %d indicators from %s to %s  |  bar_interval=%s  bundled_resample=%s",
        len(INDICATORS),
        start,
        end,
        interval_label,
        resample_label or "none",
    )
    logger.info(
        "Chunk size: %d months  |  Warmup: %d days -> %d bars",
        CHUNK_MONTHS,
        WARMUP_DAYS,
        warmup_bars,
    )

    chunks = list(date_range_chunks(start, end, CHUNK_MONTHS))
    logger.info("Total chunks: %d", len(chunks))

    all_frames = []
    total_rows = 0

    for i, (chunk_start, chunk_end) in enumerate(chunks):
        warmup_start = chunk_start - timedelta(days=WARMUP_DAYS)
        logger.info(
            "[%d/%d] Fetching %s → %s  (warmup from %s)",
            i + 1, len(chunks), chunk_start, chunk_end, warmup_start,
        )
        t0 = time.time()
        try:
            df = fetch_chunk(
                args.server,
                warmup_start,
                chunk_end,
                warmup_bars,
                interval_label,
                resample_label,
            )
        except requests.HTTPError as e:
            logger.error("  HTTP error: %s — skipping chunk", e)
            continue
        except Exception as e:
            logger.error("  Error: %s — skipping chunk", e)
            continue

        elapsed = time.time() - t0
        if df.empty:
            logger.warning("  Empty response — skipping")
            continue

        total_rows += len(df)
        all_frames.append(df)
        logger.info("  Got %d rows  (%.1fs)  cumulative: %d", len(df), elapsed, total_rows)

    if not all_frames:
        logger.error("No data fetched.")
        sys.exit(1)

    logger.info("Concatenating %d chunks ...", len(all_frames))
    full = pd.concat(all_frames, ignore_index=True)

    # De-duplicate (chunks are non-overlapping but warmup edge might overlap)
    full.drop_duplicates(subset=["timestamp_ms"], keep="last", inplace=True)
    full.sort_values("timestamp_ms", inplace=True)
    full.reset_index(drop=True, inplace=True)

    logger.info("Final shape: %s", full.shape)
    logger.info("Date range : %s → %s",
        pd.to_datetime(full["timestamp_ms"].iloc[0],  unit="ms"),
        pd.to_datetime(full["timestamp_ms"].iloc[-1], unit="ms"),
    )

    # NaN rate per column
    nan_rates = full.isnull().mean().sort_values(ascending=False)
    high_nan = nan_rates[nan_rates > 0.05]
    if not high_nan.empty:
        logger.warning("Columns with >5%% NaN:")
        for col, rate in high_nan.items():
            logger.warning("  %s: %.1f%%", col, 100 * rate)

    os.makedirs("data", exist_ok=True)
    full.to_parquet(args.out, index=False, compression="snappy")
    logger.info("Saved -> %s  (%.1f MB)", args.out,
                os.path.getsize(args.out) / 1e6)

    metadata = {
        "source": "rust_backend",
        "pipeline_stage": "raw_indicators",
        "server": args.server.rstrip("/"),
        "from": start.isoformat(),
        "to": end.isoformat(),
        "bar_interval": interval_label,
        "bundled_resample_interval": resample_label,
        "rows": int(len(full)),
        "columns": list(full.columns),
        "indicator_count": len([c for c in full.columns if c != "timestamp_ms"]),
    }
    with open(metadata_path, "w") as fh:
        json.dump(metadata, fh, indent=2)
    logger.info("Saved metadata -> %s", metadata_path)


if __name__ == "__main__":
    main()
