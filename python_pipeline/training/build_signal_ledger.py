"""
build_signal_ledger.py — Python-side signal generator for strategies not backed by Rust backtest.

Reads the raw indicator parquet, applies entry rules, simulates fixed-R exits
(1R target / 1R stop based on ATR), and joins normalized features to produce
a trade ledger compatible with train_profitability_filter.py.

Usage:
    PYTHONPATH=. python3 python_pipeline/training/build_signal_ledger.py \
      --strategy bb_mean_reversion \
      --indicators python_pipeline/data/indicators_full_5m.parquet \
      --features python_pipeline/data/features_normalized_5m.parquet \
      --from-date 2022-01-01 --to-date 2024-12-31 \
      --output python_pipeline/models/runs/bb_mean_reversion_profit_gate_5m_v1/trade_ledger.parquet
"""

import argparse
import datetime as dt
import json
import logging
import os
import sys

import numpy as np
import pandas as pd

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)


# ── Entry signal functions ───────────────────────────────────────────────────

def bb_mean_reversion_signals(df: pd.DataFrame, lookback: int = 4) -> pd.Series:
    """
    Bollinger Band mean reversion — long entry when:
      1. BB %B was below 0.15 within the last `lookback` bars (deep oversold)
      2. Current %B is rising (bouncing)
      3. RSI < 45 and rising from recent low
      4. Price is below BB middle (room to revert)
      5. Volume confirmation: CMF not deeply negative
    """
    close = df["candle.close"]
    bb_mid = df["indicator_snapshot.volatility.bb_middle_20"]
    bb_pct_b = df["indicator_snapshot.volatility.bb_pct_b_20"]
    rsi = df["indicator_snapshot.momentum.rsi_14"]
    cmf = df.get("indicator_snapshot.volume.cmf_20")

    # Condition 1: %B was deeply oversold recently
    was_oversold = pd.Series(False, index=df.index)
    for lag in range(1, lookback + 1):
        was_oversold = was_oversold | (bb_pct_b.shift(lag) < 0.15)

    # Condition 2: %B is now rising (bouncing off bottom)
    pct_b_rising = bb_pct_b > bb_pct_b.shift(1)
    pct_b_still_low = bb_pct_b < 0.5  # still below midline

    # Condition 3: RSI turning up
    prev_rsi = rsi.shift(1)
    rsi_turning = (rsi < 45) & (rsi > prev_rsi) & (rsi > 20)

    # Condition 4: below BB mid
    below_mid = close < bb_mid

    signal = was_oversold & pct_b_rising & pct_b_still_low & rsi_turning & below_mid

    # Condition 5: CMF not deeply negative (if available)
    if cmf is not None:
        signal = signal & (cmf > -0.2)

    return signal.fillna(False)


def rsi_pullback_signals(df: pd.DataFrame) -> pd.Series:
    """
    RSI Pullback in uptrend — long entry when:
      1. EMA fast > EMA slow (uptrend)
      2. RSI was below 50 within last 5 bars (pullback occurred)
      3. RSI is now rising (turning up from pullback)
      4. RSI between 30 and 55 (not a crash, not already overbought)
      5. Price above SMA 50 (macro trend confirmation)
    ADX filtering left to the ML model.
    """
    ema_fast = df["ema_fast"]
    ema_slow = df["ema_slow"]
    rsi = df["indicator_snapshot.momentum.rsi_14"]
    close = df["candle.close"]
    sma50 = df.get("indicator_snapshot.trend.sma_50")

    # Condition 1: uptrend
    trend_up = ema_fast > ema_slow

    # Condition 2: RSI was below 50 recently (pullback occurred)
    was_pulled_back = pd.Series(False, index=df.index)
    for lag in range(1, 6):
        was_pulled_back = was_pulled_back | (rsi.shift(lag) < 50)

    # Condition 3: RSI rising
    rsi_rising = rsi > rsi.shift(1)

    # Condition 4: RSI in the sweet spot
    rsi_zone = (rsi > 30) & (rsi < 55)

    # Condition 5: above SMA 50
    if sma50 is not None:
        above_sma50 = close > sma50
    else:
        above_sma50 = pd.Series(True, index=df.index)

    signal = trend_up & was_pulled_back & rsi_rising & rsi_zone & above_sma50
    return signal.fillna(False)


def supertrend_adx_signals(df: pd.DataFrame) -> pd.Series:
    """
    Supertrend flip + ADX trend confirmation — long entry when:
      1. Supertrend just flipped to long (was short previous bar)
      2. ADX > 20 (trending market)
      3. DI+ > DI- (bulls dominating)
      4. Price above EMA slow (macro trend alignment)
      5. RSI not overbought (< 70)
    """
    st_long = df["indicator_snapshot.volatility.supertrend_long"]
    adx = df["indicator_snapshot.directional.adx_14"]
    di_plus = df["indicator_snapshot.directional.di_plus"]
    di_minus = df["indicator_snapshot.directional.di_minus"]
    ema_slow = df["ema_slow"]
    close = df["candle.close"]
    rsi = df["indicator_snapshot.momentum.rsi_14"]

    # Condition 1: supertrend just flipped to long
    st_flip = (st_long == True) & (st_long.shift(1) == False)

    # Condition 2: ADX confirms trend
    trending = adx > 20

    # Condition 3: DI+ > DI-
    di_bullish = di_plus > di_minus

    # Condition 4: above EMA slow
    above_ema = close > ema_slow

    # Condition 5: not overbought
    not_overbought = rsi < 70

    signal = st_flip & trending & di_bullish & above_ema & not_overbought
    return signal.fillna(False)


def donchian_breakout_signals(df: pd.DataFrame) -> pd.Series:
    """
    Donchian channel breakout — long entry when:
      1. Close breaks above Donchian upper band (new 20-bar high)
      2. Volume above average (vol_ratio > 1.2)
      3. ADX > 22 (trending, not choppy)
      4. Supertrend is long (trend alignment)
      5. RSI > 50 and < 75 (momentum confirmation but not overbought)
    """
    close = df["candle.close"]
    don_upper = df["indicator_snapshot.volatility.donchian_upper_20"]
    don_upper_prev = don_upper.shift(1)
    vol_ratio = df["vol_ratio"]
    adx = df["indicator_snapshot.directional.adx_14"]
    st_long = df["indicator_snapshot.volatility.supertrend_long"]
    rsi = df["indicator_snapshot.momentum.rsi_14"]

    # Condition 1: breakout — close above previous Donchian upper
    breakout = close > don_upper_prev

    # Condition 2: volume confirmation
    vol_confirm = vol_ratio > 1.2

    # Condition 3: trending
    trending = adx > 22

    # Condition 4: supertrend aligned
    trend_aligned = st_long == True

    # Condition 5: momentum range
    rsi_ok = (rsi > 50) & (rsi < 75)

    signal = breakout & vol_confirm & trending & trend_aligned & rsi_ok
    return signal.fillna(False)


def macd_crossover_signals(df: pd.DataFrame) -> pd.Series:
    """
    MACD histogram crosses zero from below — long entry when:
      1. MACD histogram crosses from negative to positive (momentum shift)
      2. EMA fast > EMA slow (trend alignment) — optional, let ML decide
    Intentionally broad to give the ML model maximum data to learn from.
    """
    macd_hist = df["indicator_snapshot.momentum.macd_hist"]
    signal = (macd_hist > 0) & (macd_hist.shift(1) <= 0)
    return signal.fillna(False)


STRATEGY_SIGNALS = {
    "bb_mean_reversion": bb_mean_reversion_signals,
    "rsi_pullback": rsi_pullback_signals,
    "supertrend_adx": supertrend_adx_signals,
    "donchian_breakout": donchian_breakout_signals,
    "macd_crossover": macd_crossover_signals,
}


# ── Exit simulation ─────────────────────────────────────────────────────────

def simulate_exits(
    df: pd.DataFrame,
    signals: pd.Series,
    atr_col: str = "atr",
    risk_multiple_target: float = 2.0,
    risk_multiple_stop: float = 1.0,
    max_hold_bars: int = 20,
    entry_fee_bps: float = 10.0,
    exit_fee_bps: float = 10.0,
    entry_slippage_bps: float = 2.0,
    exit_slippage_bps: float = 2.0,
    use_bb_target: bool = False,
) -> pd.DataFrame:
    """
    Simulate long trades using close prices and ATR-based stop/target.
    If use_bb_target=True, target is the BB midline instead of fixed ATR multiple.
    Returns a DataFrame of resolved trades with net_r.
    """
    close = df["candle.close"].values
    atr = df[atr_col].values
    ts = df["timestamp_ms"].values
    signal_indices = np.where(signals.values)[0]

    # Use high/low for realistic stop/target detection
    has_hl = "candle.high" in df.columns and "candle.low" in df.columns
    high = df["candle.high"].values if has_hl else close
    low = df["candle.low"].values if has_hl else close

    bb_mid = None
    if use_bb_target and "indicator_snapshot.volatility.bb_middle_20" in df.columns:
        bb_mid = df["indicator_snapshot.volatility.bb_middle_20"].values

    total_cost_bps = entry_fee_bps + exit_fee_bps + entry_slippage_bps + exit_slippage_bps

    trades = []
    i = 0
    while i < len(signal_indices):
        idx = signal_indices[i]
        if idx + 1 >= len(close) or np.isnan(atr[idx]) or atr[idx] <= 0:
            i += 1
            continue

        entry_price = close[idx]
        stop_distance = risk_multiple_stop * atr[idx]
        risk = stop_distance  # 1R = distance to stop
        stop_price = entry_price - stop_distance

        if use_bb_target and bb_mid is not None and not np.isnan(bb_mid[idx]):
            target_price = bb_mid[idx]
            if target_price <= entry_price:
                target_price = entry_price + risk_multiple_target * atr[idx]
        else:
            target_price = entry_price + risk_multiple_target * atr[idx]

        exit_price = None
        exit_reason = None
        exit_idx = None

        for j in range(idx + 1, min(idx + 1 + max_hold_bars, len(close))):
            hit_stop = low[j] <= stop_price
            hit_target = high[j] >= target_price

            if hit_stop and hit_target:
                # Both hit same bar — assume stop hit first (conservative)
                exit_price = stop_price
                exit_reason = "stop"
                exit_idx = j
                break
            if hit_stop:
                exit_price = stop_price
                exit_reason = "stop"
                exit_idx = j
                break
            if hit_target:
                exit_price = target_price
                exit_reason = "target"
                exit_idx = j
                break

        if exit_price is None:
            exit_idx = min(idx + max_hold_bars, len(close) - 1)
            exit_price = close[exit_idx]
            exit_reason = "timeout"

        gross_r = (exit_price - entry_price) / risk
        fee_cost_r = (total_cost_bps / 10000.0) * entry_price / risk
        net_r = gross_r - fee_cost_r

        trades.append({
            "signal_close_time_ms": int(ts[idx]),
            "entry_bar_index": int(idx),
            "exit_bar_index": int(exit_idx),
            "entry_price_fill": float(entry_price),
            "exit_price_fill": float(exit_price),
            "stop_price": float(stop_price),
            "target_price": float(target_price),
            "atr_at_signal": float(risk),
            "bars_held": int(exit_idx - idx),
            "exit_reason": exit_reason,
            "gross_r": float(gross_r),
            "net_r": float(net_r),
            "profitable": int(net_r > 0),
            "entry_fee_bps": entry_fee_bps,
            "exit_fee_bps": exit_fee_bps,
            "entry_slippage_bps": entry_slippage_bps,
            "exit_slippage_bps": exit_slippage_bps,
        })

        # Skip signals that overlap with this trade
        skip_until = exit_idx
        i += 1
        while i < len(signal_indices) and signal_indices[i] <= skip_until:
            i += 1

    return pd.DataFrame(trades)


# ── Forward return labeling ──────────────────────────────────────────────────

def forward_return_ledger(
    df: pd.DataFrame,
    signals: pd.Series,
    horizon: int = 12,
    total_cost_bps: float = 24.0,
    allow_overlap: bool = False,
) -> pd.DataFrame:
    """
    Label each signal bar with the forward return at `horizon` bars.
    Exit price = close[idx + horizon] (horizon exit, realistic).
    net_r = (exit - entry) / atr - cost_in_r_terms

    If allow_overlap is False, subsequent signals within `horizon` bars are
    skipped (single-position backtest). If True, every signal becomes a row
    (each position treated independently — use for ML labelling where you
    want every signal to carry its own label).
    """
    close = df["candle.close"].values
    atr = df["atr"].values
    ts = df["timestamp_ms"].values
    signal_indices = np.where(signals.values)[0]

    trades = []
    i = 0
    while i < len(signal_indices):
        idx = signal_indices[i]
        if idx + 1 >= len(close) or np.isnan(atr[idx]) or atr[idx] <= 0:
            i += 1
            continue

        entry_price = close[idx]
        risk = atr[idx]
        end = min(idx + 1 + horizon, len(close))

        if end <= idx + 1:
            i += 1
            continue

        future_closes = close[idx + 1:end]
        best_exit = future_closes.max()
        worst_exit = future_closes.min()
        final_exit = close[end - 1]

        # Use the close at horizon as actual exit (realistic)
        exit_price = final_exit
        exit_idx = end - 1

        gross_r = (exit_price - entry_price) / risk
        fee_cost_r = (total_cost_bps / 10000.0) * entry_price / risk
        net_r = gross_r - fee_cost_r

        trades.append({
            "signal_close_time_ms": int(ts[idx]),
            "entry_bar_index": int(idx),
            "exit_bar_index": int(exit_idx),
            "entry_price_fill": float(entry_price),
            "exit_price_fill": float(exit_price),
            "stop_price": float(entry_price - 2 * risk),
            "target_price": float(best_exit),
            "atr_at_signal": float(risk),
            "bars_held": int(exit_idx - idx),
            "exit_reason": "horizon",
            "gross_r": float(gross_r),
            "fwd_ret_r": float(gross_r),
            "net_r": float(net_r),
            "profitable": int(net_r > 0),
            "entry_fee_bps": total_cost_bps / 4,
            "exit_fee_bps": total_cost_bps / 4,
            "entry_slippage_bps": total_cost_bps / 4,
            "exit_slippage_bps": total_cost_bps / 4,
        })

        if allow_overlap:
            i += 1
        else:
            skip_until = min(idx + horizon, len(close) - 1)
            i += 1
            while i < len(signal_indices) and signal_indices[i] <= skip_until:
                i += 1

    return pd.DataFrame(trades)


# ── Feature join ─────────────────────────────────────────────────────────────

def join_features(trades_df: pd.DataFrame, features_path: str):
    feats = pd.read_parquet(features_path).sort_values("timestamp_ms").reset_index(drop=True)
    feat_cols = [c for c in feats.columns if c != "timestamp_ms"]

    joined = pd.merge(
        trades_df,
        feats,
        left_on="signal_close_time_ms",
        right_on="timestamp_ms",
        how="left",
    )
    before = len(joined)
    joined.dropna(subset=feat_cols, inplace=True)
    joined.reset_index(drop=True, inplace=True)
    logger.info("Feature join: %d / %d rows after NaN filtering", len(joined), before)
    return joined, feat_cols


# ── Main ─────────────────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser()
    p.add_argument("--strategy", required=True, choices=list(STRATEGY_SIGNALS.keys()))
    p.add_argument("--indicators", required=True, help="Raw indicator parquet")
    p.add_argument("--features", required=True, help="Normalized features parquet")
    p.add_argument("--from-date", default=None)
    p.add_argument("--to-date", default=None)
    p.add_argument("--max-hold-bars", type=int, default=20)
    p.add_argument("--risk-multiple-target", type=float, default=2.0)
    p.add_argument("--risk-multiple-stop", type=float, default=1.0)
    p.add_argument("--entry-fee-bps", type=float, default=10.0)
    p.add_argument("--exit-fee-bps", type=float, default=10.0)
    p.add_argument("--entry-slippage-bps", type=float, default=2.0)
    p.add_argument("--exit-slippage-bps", type=float, default=2.0)
    p.add_argument("--use-bb-target", action="store_true",
                   help="Use BB midline as target instead of fixed ATR multiple")
    p.add_argument("--use-forward-returns", action="store_true",
                   help="Use forward return labeling instead of stop/target simulation")
    p.add_argument("--allow-overlap", action="store_true",
                   help="Allow overlapping trades (one row per signal; for ML labelling). "
                        "Default: skip signals within horizon of a taken trade.")
    p.add_argument("--max-cost-r", type=float, default=0.5,
                   help="Max fee cost in R terms; drop trades where cost > this (default 0.5)")
    p.add_argument("--output", required=True)
    return p.parse_args()


def main():
    args = parse_args()

    logger.info("Loading indicators from %s", args.indicators)
    df = pd.read_parquet(args.indicators).sort_values("timestamp_ms").reset_index(drop=True)
    logger.info("Loaded %d rows", len(df))

    if args.from_date:
        from_ms = int(dt.datetime.fromisoformat(args.from_date).replace(
            tzinfo=dt.timezone.utc).timestamp() * 1000)
        df = df[df["timestamp_ms"] >= from_ms]
    if args.to_date:
        to_ms = int((dt.datetime.fromisoformat(args.to_date).replace(
            tzinfo=dt.timezone.utc) + dt.timedelta(days=1)).timestamp() * 1000)
        df = df[df["timestamp_ms"] < to_ms]
    df = df.reset_index(drop=True)
    logger.info("Date-filtered to %d rows", len(df))

    signal_fn = STRATEGY_SIGNALS[args.strategy]
    signals = signal_fn(df)
    logger.info("Raw signals: %d", signals.sum())

    if signals.sum() == 0:
        raise SystemExit("no entry signals generated")

    if args.use_forward_returns:
        trades_df = forward_return_ledger(
            df, signals,
            horizon=args.max_hold_bars,
            total_cost_bps=args.entry_fee_bps + args.exit_fee_bps +
                           args.entry_slippage_bps + args.exit_slippage_bps,
            allow_overlap=args.allow_overlap,
        )
    else:
        trades_df = simulate_exits(
            df, signals,
            max_hold_bars=args.max_hold_bars,
            risk_multiple_target=args.risk_multiple_target,
            risk_multiple_stop=args.risk_multiple_stop,
            entry_fee_bps=args.entry_fee_bps,
            exit_fee_bps=args.exit_fee_bps,
            entry_slippage_bps=args.entry_slippage_bps,
            exit_slippage_bps=args.exit_slippage_bps,
            use_bb_target=args.use_bb_target,
        )
    trades_df["strategy_id"] = args.strategy
    logger.info("Resolved trades (before cost filter): %d", len(trades_df))

    # Filter out trades where fee cost is too large relative to risk
    if args.max_cost_r and args.max_cost_r > 0 and not trades_df.empty:
        total_cost_bps = args.entry_fee_bps + args.exit_fee_bps + args.entry_slippage_bps + args.exit_slippage_bps
        cost_r = (total_cost_bps / 10000.0) * trades_df["entry_price_fill"] / trades_df["atr_at_signal"]
        before = len(trades_df)
        trades_df = trades_df[cost_r <= args.max_cost_r].reset_index(drop=True)
        logger.info("Cost filter (max %.2fR): kept %d / %d trades", args.max_cost_r, len(trades_df), before)

    win_rate = trades_df["profitable"].mean()
    avg_r = trades_df["net_r"].mean()
    logger.info("Win rate: %.1f%%  Avg net_r: %.4f", 100 * win_rate, avg_r)

    joined, feat_cols = join_features(trades_df, args.features)
    if joined.empty:
        raise SystemExit("all trades dropped after feature join")

    os.makedirs(os.path.dirname(args.output), exist_ok=True)
    joined.to_parquet(args.output, index=False)
    logger.info("Wrote %d trade rows -> %s", len(joined), args.output)

    summary_path = os.path.splitext(args.output)[0] + ".summary.json"
    with open(summary_path, "w") as fh:
        json.dump({
            "strategy_id": args.strategy,
            "feature_count": len(feat_cols),
            "trade_rows": len(trades_df),
            "output_rows": len(joined),
            "win_rate": float(win_rate),
            "avg_net_r": float(avg_r),
        }, fh, indent=2)
    logger.info("Wrote summary -> %s", summary_path)


if __name__ == "__main__":
    main()
