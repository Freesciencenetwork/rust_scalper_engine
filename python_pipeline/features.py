"""
features.py — Indicator computation and target creation.

Design principles:
  - Every indicator uses only past/current data (no lookahead).
  - All rolling operations use .shift(1) on the raw series before the window
    so that the *current* bar is excluded from rolling statistics —
    preventing any within-bar leakage.
  - The public API is two functions:
      add_features(df)        -> df with feature columns appended
      add_target(df, h, thr)  -> df with 'target' column appended
"""
from typing import Dict, List, Optional, Tuple

import logging
import numpy as np
import pandas as pd

import config

logger = logging.getLogger(__name__)

# ────────────────────────────────────────────────────────────────────────────
# Internal helpers
# ────────────────────────────────────────────────────────────────────────────

def _ema(series: pd.Series, span: int) -> pd.Series:
    """Exponential moving average.  adjust=False matches TradingView / TA-Lib."""
    return series.ewm(span=span, adjust=False).mean()


def _rsi(close: pd.Series, period: int = 14) -> pd.Series:
    """
    Classic Wilder RSI.

    Uses EMA with com = period-1  (equivalent to alpha = 1/period),
    which matches the original Wilder smoothing method.
    Computed only on past bars: we shift close by 1 first, meaning
    the RSI at bar t is computed from bars [t-period … t-1].
    """
    # delta computed on past prices only — shift by 1 so bar t sees t-1
    delta = close.shift(1).diff()
    gain  = delta.clip(lower=0)
    loss  = (-delta).clip(lower=0)

    avg_gain = gain.ewm(com=period - 1, min_periods=period, adjust=False).mean()
    avg_loss = loss.ewm(com=period - 1, min_periods=period, adjust=False).mean()

    rs  = avg_gain / avg_loss.replace(0, np.nan)
    rsi = 100 - (100 / (1 + rs))
    return rsi


def _macd(
    close: pd.Series,
    fast: int   = config.MACD_FAST,
    slow: int   = config.MACD_SLOW,
    signal: int = config.MACD_SIGNAL,
) -> Tuple[pd.Series, pd.Series, pd.Series]:
    """
    Returns (macd_line, signal_line, histogram).
    All computed on past bars (shift by 1 before EMA).
    """
    past_close  = close.shift(1)           # only look at t-1 and earlier
    ema_fast    = _ema(past_close, fast)
    ema_slow    = _ema(past_close, slow)
    macd_line   = ema_fast - ema_slow
    signal_line = _ema(macd_line, signal)
    histogram   = macd_line - signal_line
    return macd_line, signal_line, histogram


# ────────────────────────────────────────────────────────────────────────────
# Public API
# ────────────────────────────────────────────────────────────────────────────

def add_features(df: pd.DataFrame) -> pd.DataFrame:
    """
    Compute technical indicators from raw OHLCV candles and append them as
    new columns.  The original columns are preserved.

    All features are computed exclusively from data available at bar t
    (i.e., [0 … t-1] for rolling stats, [t] for the current close).
    The close at bar t is considered known because the bar has already closed.

    Parameters
    ----------
    df : pd.DataFrame
        Must contain columns: open, high, low, close, volume.
        Must be sorted ascending by timestamp before calling.

    Returns
    -------
    pd.DataFrame
        Same rows, extra feature columns appended.  Rows that cannot be
        computed (due to insufficient history) will have NaN — they are
        dropped later during target creation.
    """
    df = df.copy()
    close  = df["close"]
    volume = df["volume"]

    # ── 1. Price returns ─────────────────────────────────────────────────────
    # pct_change already looks back, so close[t] / close[t-1] - 1 is safe.
    df["ret_1"]  = close.pct_change(1)
    df["ret_3"]  = close.pct_change(3)
    df["ret_5"]  = close.pct_change(5)

    # ── 2. Rolling mean & volatility ─────────────────────────────────────────
    # shift(1) ensures bar t does NOT include itself in the rolling window.
    win_mean = config.ROLLING_MEAN_WIN
    win_std  = config.ROLLING_STD_WIN

    df["roll_mean"] = close.shift(1).rolling(win_mean).mean()
    df["roll_std"]  = close.shift(1).rolling(win_std).std()

    # Normalise close by the rolling mean to make it comparable across regimes
    df["close_to_mean"] = close / df["roll_mean"] - 1

    # ── 3. RSI ───────────────────────────────────────────────────────────────
    df["rsi"] = _rsi(close, period=config.RSI_PERIOD)

    # ── 4. MACD ──────────────────────────────────────────────────────────────
    df["macd_line"], df["macd_signal"], df["macd_hist"] = _macd(
        close,
        fast=config.MACD_FAST,
        slow=config.MACD_SLOW,
        signal=config.MACD_SIGNAL,
    )

    # ── 5. Volume features ───────────────────────────────────────────────────
    df["vol_change"]  = volume.pct_change(1)

    # Z-score of volume against recent history — shift(1) for no-lookahead
    vol_mean = volume.shift(1).rolling(config.VOL_ZSCORE_WIN).mean()
    vol_std  = volume.shift(1).rolling(config.VOL_ZSCORE_WIN).std()
    df["vol_zscore"] = (volume - vol_mean) / vol_std.replace(0, np.nan)

    # ── 6. EMA spread ────────────────────────────────────────────────────────
    # The spread between fast and slow EMA encodes momentum direction.
    df["ema_fast"]   = _ema(close.shift(1), config.EMA_FAST)
    df["ema_slow"]   = _ema(close.shift(1), config.EMA_SLOW)
    df["ema_spread"] = (df["ema_fast"] - df["ema_slow"]) / df["ema_slow"]

    # Replace any inf values (e.g. from division by zero in vol_zscore or
    # ema_spread when std == 0) with NaN so they are caught by the downstream
    # dropna() in add_target() and never reach the model.
    df.replace([np.inf, -np.inf], np.nan, inplace=True)

    logger.info("Feature engineering complete.  Shape: %s", df.shape)
    return df


def add_extended_features(df: pd.DataFrame) -> pd.DataFrame:
    """
    Extend a DataFrame that already has base features (from add_features)
    with additional signals useful for the vol-scaled binary tasks.

    New features
    ------------
    Lagged returns        : ret_2, ret_10, ret_15, ret_30
    High-low range        : hl_range  = (high - low) / close.shift(1)
    Realized volatility   : realized_vol_20, realized_vol_60
                            = rolling std of ret_1 over past N bars
    Momentum              : mom_10, mom_20  = sum of ret_1 over past N bars
    Mean-reversion dist   : dist_ema20, dist_ema50
                            = close / ema(N) - 1  (using past close)
    Time-of-day features  : hour_of_day, day_of_week
                            (from timestamp column; zero-filled if absent)

    Stubs (NOT computed — require external data sources)
    ----------------------------------------------------
    funding_rate          : perpetual futures funding rate
    open_interest         : aggregate OI from exchange
    bid_ask_spread        : level-2 order book
    order_flow_imbalance  : signed trade imbalance from tick data

    Parameters
    ----------
    df : pd.DataFrame
        Must already have base features + timestamp column.

    Returns
    -------
    pd.DataFrame with extended feature columns appended.
    """
    df = df.copy()
    close  = df["close"]
    ret_1  = df["ret_1"]   # already computed in add_features

    # ── Additional lagged returns ────────────────────────────────────────────
    df["ret_2"]  = close.pct_change(2)
    df["ret_10"] = close.pct_change(10)
    df["ret_15"] = close.pct_change(15)
    df["ret_30"] = close.pct_change(30)

    # ── High-low range (normalized ATR proxy) ────────────────────────────────
    # Using shift(1) on the denominator ensures no within-bar leakage.
    prev_close = close.shift(1)
    df["hl_range"] = (df["high"] - df["low"]) / prev_close

    # ── Realized volatility ──────────────────────────────────────────────────
    # shift(1) on ret_1 so the current bar's return is excluded from the window.
    past_ret = ret_1.shift(1)
    df["realized_vol_20"] = past_ret.rolling(20).std()
    df["realized_vol_60"] = past_ret.rolling(60).std()

    # ── Momentum (sum of past N returns) ────────────────────────────────────
    df["mom_10"] = past_ret.rolling(10).sum()
    df["mom_20"] = past_ret.rolling(20).sum()

    # ── Mean-reversion distance from EMA ────────────────────────────────────
    ema20 = _ema(close.shift(1), 20)
    ema50 = _ema(close.shift(1), 50)
    df["dist_ema20"] = close / ema20 - 1
    df["dist_ema50"] = close / ema50 - 1

    # ── Time-of-day and day-of-week ──────────────────────────────────────────
    # Captures intraday seasonality (e.g. Asian/European/US session effects)
    # and day-of-week patterns (e.g. weekend low-liquidity effects).
    if "timestamp" in df.columns:
        ts = pd.to_datetime(df["timestamp"])
        df["hour_of_day"]  = ts.dt.hour.astype(np.float32)
        df["day_of_week"]  = ts.dt.dayofweek.astype(np.float32)
    else:
        df["hour_of_day"] = 0.0
        df["day_of_week"] = 0.0

    df.replace([np.inf, -np.inf], np.nan, inplace=True)
    return df


# ── Canonical feature column lists ────────────────────────────────────────
# FEATURE_COLUMNS   : original 15, used by legacy train.py
# EXTENDED_FEATURE_COLUMNS : additional 14, used by Task A / Task B
# ALL_FEATURE_COLUMNS : union — the recommended set for new experiments
FEATURE_COLUMNS: List[str] = [
    "ret_1",
    "ret_3",
    "ret_5",
    "roll_mean",
    "roll_std",
    "close_to_mean",
    "rsi",
    "macd_line",
    "macd_signal",
    "macd_hist",
    "vol_change",
    "vol_zscore",
    "ema_fast",
    "ema_slow",
    "ema_spread",
]

EXTENDED_FEATURE_COLUMNS: List[str] = [
    "ret_2",
    "ret_10",
    "ret_15",
    "ret_30",
    "hl_range",
    "realized_vol_20",
    "realized_vol_60",
    "mom_10",
    "mom_20",
    "dist_ema20",
    "dist_ema50",
    "hour_of_day",
    "day_of_week",
]

ALL_FEATURE_COLUMNS: List[str] = FEATURE_COLUMNS + EXTENDED_FEATURE_COLUMNS


def add_target(
    df: pd.DataFrame,
    horizon: int   = config.HORIZON,
    threshold: float = config.THRESHOLD,
) -> pd.DataFrame:
    """
    Create a 3-class directional label from the forward return.

    future_return = (close[t+h] - close[t]) / close[t]

    Labels:
      0 (DOWN) : future_return < -threshold
      1 (FLAT) : -threshold <= future_return <= threshold
      2 (UP)   : future_return > threshold

    The shift is *negative* (.shift(-h)) to look forward — this is intentional
    and does NOT cause leakage: the target is what we want the model to predict,
    not a feature.  Features must never use future data.

    Parameters
    ----------
    df : pd.DataFrame
        Must contain 'close' and all feature columns already added.
    horizon : int
        Number of bars ahead to compute the return.
    threshold : float
        Minimum absolute return to label as UP or DOWN.

    Returns
    -------
    pd.DataFrame
        Same DataFrame with 'future_return' and 'target' columns added.
        Rows where future_return is NaN (last *horizon* rows) are dropped.
    """
    df = df.copy()

    future_close       = df["close"].shift(-horizon)
    df["future_return"] = (future_close - df["close"]) / df["close"]

    # 3-class label
    df["target"] = config.CLASS_FLAT   # default: flat
    df.loc[df["future_return"] < -threshold, "target"] = config.CLASS_DOWN
    df.loc[df["future_return"] >  threshold, "target"] = config.CLASS_UP

    # ── Drop rows that cannot have a valid label or valid features ──────────
    # Rows where future data doesn't exist (tail) and rows where indicators
    # haven't warmed up yet (head) both land here.
    before = len(df)
    df.dropna(subset=FEATURE_COLUMNS + ["future_return"], inplace=True)
    df.reset_index(drop=True, inplace=True)
    after = len(df)

    logger.info(
        "Target created (horizon=%d, threshold=%.4f).  "
        "Dropped %d NaN rows, %d usable rows remain.",
        horizon, threshold, before - after, after,
    )

    # Log class distribution so the user can spot severe imbalance early
    dist = df["target"].value_counts().sort_index()
    logger.info(
        "Class distribution — DOWN:%d  FLAT:%d  UP:%d",
        dist.get(config.CLASS_DOWN, 0),
        dist.get(config.CLASS_FLAT, 0),
        dist.get(config.CLASS_UP,   0),
    )

    return df
