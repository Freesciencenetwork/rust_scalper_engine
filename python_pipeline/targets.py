"""
targets.py — Volatility-scaled binary target creation.

Replaces the fixed-threshold 3-class approach with targets that adapt to
current market volatility.  This ensures the labels mean the same thing
(a "significant" move) regardless of whether BTC is at $100 or $100,000.

Formula
-------
  ret_1[t]        = (close[t] - close[t-1]) / close[t-1]
  rolling_vol[t]  = std( ret_1[t-VOL_WIN : t] )   <- strictly past data
  future_ret[t]   = (close[t+H] - close[t]) / close[t]

  is_move[t]      = 1  if  |future_ret[t]|  >  K * rolling_vol[t]
                  = 0  otherwise

  direction[t]    = 1 (UP)   if is_move[t]==1 and future_ret[t] > 0
                  = 0 (DOWN) if is_move[t]==1 and future_ret[t] < 0
                  = NaN      if is_move[t]==0

Why volatility-scaling?
  A 0.1% move is huge when BTC is $100 (2012) but trivial when BTC is
  $100,000 (2024).  A fixed threshold produces structurally different label
  distributions across regimes, making any trained model regime-specific.
  Rolling-vol scaling removes this bias and produces a consistent "one sigma
  event" definition throughout the entire history.

No-lookahead guarantee
  rolling_vol[t] uses ret_1 shifted by 1 before the rolling window, so the
  current bar's return is NOT included in the volatility estimate.
  future_ret[t]  uses close.shift(-H), which is the TARGET, not a feature —
  this is by design and does not constitute leakage.
"""

import logging
import numpy as np
import pandas as pd

import config
from features import ALL_FEATURE_COLUMNS

logger = logging.getLogger(__name__)


def make_vol_scaled_targets(
    df: pd.DataFrame,
    horizon: int,
    k: float,
    vol_win: int = None,    # kept for API compatibility — no longer used
    feature_cols=None,
) -> pd.DataFrame:
    """
    Attach volatility-scaled binary targets to a feature-enriched DataFrame.

    Parameters
    ----------
    df : pd.DataFrame
        Must contain 'close' and 'atr_pct' (Rust-computed ATR percentage).
    horizon : int
        Bars ahead for the future return.
    k : float
        Multiplier on atr_pct.  k=1.0 -> any move > 1 × ATR = MOVE.
    vol_win : int
        Unused — retained for call-site compatibility.  Previously controlled
        a Python rolling-std window; replaced by the Rust atr_pct column.
    feature_cols : list of str, optional
        Unused — retained for call-site compatibility.

    Returns
    -------
    pd.DataFrame
        Same rows plus:
          future_return  : raw forward return  (label — forward-looking by design)
          atr_threshold  : k × atr_pct.shift(1) used as the MOVE threshold
          is_move        : 1 if |future_return| > atr_threshold, else 0
          direction      : 1=UP, 0=DOWN, NaN=NO_MOVE

        Rows with NaN in future_return or atr_pct are dropped.
        Index is reset.
    """
    df = df.copy()

    # ── Volatility threshold from Rust ───────────────────────────────────────
    # trend_hist_vol_logrets_20 is the Rust-computed 20-bar historical
    # volatility of log returns.  shift(1) ensures bar t uses only past data.
    # k≈3.7 with this feature gives a MOVE rate comparable to the original
    # rolling_std(ret_1, 60) with k=1.0 (~44% MOVE bars).
    df["atr_threshold"] = k * df["trend_hist_vol_logrets_20"].shift(1)

    # ── Future return (label — forward-looking by design) ────────────────────
    future_close        = df["close"].shift(-horizon)
    df["future_return"] = (future_close - df["close"]) / df["close"]

    # ── MOVE / NO_MOVE label ─────────────────────────────────────────────────
    abs_fut   = df["future_return"].abs()
    threshold = df["atr_threshold"]
    df["is_move"] = np.where(
        abs_fut > threshold,
        config.TASK_A_LABEL_MOVE,
        config.TASK_A_LABEL_NOMOVE,
    )
    df["is_move"] = df["is_move"].astype("Int64")

    # ── Direction label (UP=1 / DOWN=0, NaN for NO_MOVE) ────────────────────
    df["direction"] = np.nan
    move_mask = df["is_move"] == config.TASK_A_LABEL_MOVE
    df.loc[move_mask & (df["future_return"] > 0), "direction"] = float(config.TASK_B_LABEL_UP)
    df.loc[move_mask & (df["future_return"] < 0), "direction"] = float(config.TASK_B_LABEL_DOWN)

    # ── Drop rows with undefined labels ──────────────────────────────────────
    before = len(df)
    df.dropna(subset=["future_return", "atr_threshold"], inplace=True)
    df.reset_index(drop=True, inplace=True)
    after = len(df)

    # ── Diagnostics ──────────────────────────────────────────────────────────
    move_count   = int((df["is_move"] == 1).sum())
    nomove_count = int((df["is_move"] == 0).sum())
    total        = len(df)
    move_pct     = 100 * move_count / total if total else 0

    logger.info(
        "Vol-scaled targets (horizon=%d, k=%.2f, threshold=atr_pct×%.2f): "
        "dropped %d NaN rows, %d usable rows",
        horizon, k, k, before - after, total,
    )
    logger.info(
        "  MOVE: %d (%.1f%%)  NO_MOVE: %d (%.1f%%)",
        move_count, move_pct, nomove_count, 100 - move_pct,
    )

    if move_pct < 5:
        logger.warning(
            "MOVE class is only %.1f%% of data. "
            "Consider lowering K (currently %.2f) or increasing horizon.",
            move_pct, k,
        )
    if move_pct > 60:
        logger.warning(
            "MOVE class is %.1f%% of data. "
            "Consider raising K (currently %.2f) or decreasing horizon.",
            move_pct, k,
        )

    return df
