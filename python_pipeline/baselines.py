"""
baselines.py — Trivial baselines that any useful model must beat.

All baselines are pure functions: they take (X, y_true, feature_cols) and
return a y_pred array.  This makes them easy to drop into any evaluation loop.

Task A baselines  (MOVE vs NO_MOVE)
  - always_nomove       : predict 0 for every bar
  - always_move         : predict 1 for every bar
  - vol_threshold       : predict MOVE when realized_vol > median(realized_vol_train)
                          (a simple volatility-regime heuristic)

Task B baselines  (UP vs DOWN on MOVE bars)
  - always_up           : predict 1 for every bar
  - always_down         : predict 0 for every bar
  - prev_return_sign    : predict UP if the most recent 1-bar return is positive
  - rolling_momentum    : predict UP if the 10-bar momentum is positive

Why do we need baselines?
  Without baselines, "46% accuracy" is meaningless.  A model must beat the
  cheapest possible predictor before we can claim it found signal.
"""
from typing import Dict, List, Optional, Tuple

import numpy as np


# ────────────────────────────────────────────────────────────────────────────
# Task A — MOVE vs NO_MOVE
# ────────────────────────────────────────────────────────────────────────────

def always_nomove(n: int) -> np.ndarray:
    """Predict NO_MOVE (0) for every bar.  Best strategy when MOVE is rare."""
    return np.zeros(n, dtype=int)


def always_move(n: int) -> np.ndarray:
    """Predict MOVE (1) for every bar."""
    return np.ones(n, dtype=int)


def vol_threshold_baseline(
    X_train: np.ndarray,
    X_test:  np.ndarray,
    feature_cols: List[str],
) -> np.ndarray:
    """
    Predict MOVE when the current realized_vol_20 exceeds the training-set
    median.  This heuristic tests whether simple volatility regime-detection
    is enough to identify MOVE bars without any directional model.

    Falls back to always_nomove if realized_vol_20 is not in feature_cols.
    """
    if "realized_vol_20" not in feature_cols:
        return always_nomove(len(X_test))

    idx = feature_cols.index("realized_vol_20")
    median_vol = np.nanmedian(X_train[:, idx])
    return (X_test[:, idx] > median_vol).astype(int)


# ────────────────────────────────────────────────────────────────────────────
# Task B — UP vs DOWN
# ────────────────────────────────────────────────────────────────────────────

def always_up(n: int) -> np.ndarray:
    """Predict UP (1) for every MOVE bar."""
    return np.ones(n, dtype=int)


def always_down(n: int) -> np.ndarray:
    """Predict DOWN (0) for every MOVE bar."""
    return np.zeros(n, dtype=int)


def prev_return_sign(
    X_test: np.ndarray,
    feature_cols: List[str],
) -> np.ndarray:
    """
    Predict UP if the most recent 1-bar return is positive (momentum continuation),
    DOWN otherwise.

    This tests whether raw price momentum (the simplest possible signal) has
    directional predictive power at the given horizon.
    """
    if "ret_1" not in feature_cols:
        return always_up(len(X_test))

    idx = feature_cols.index("ret_1")
    return (X_test[:, idx] > 0).astype(int)


def rolling_momentum_sign(
    X_test: np.ndarray,
    feature_cols: List[str],
) -> np.ndarray:
    """
    Predict UP if a momentum proxy is positive.

    Tries mom_10, then momentum_roc_10, then ret_5 in order.
    Falls back to always_up if none are available (e.g. after resampling
    removes 1m-only features).
    """
    for col in ("mom_10", "momentum_roc_10", "ret_5"):
        if col in feature_cols:
            idx = feature_cols.index(col)
            return (X_test[:, idx] > 0).astype(int)
    return always_up(len(X_test))


# ────────────────────────────────────────────────────────────────────────────
# Convenience: run all baselines for a given task
# ────────────────────────────────────────────────────────────────────────────

def run_task_a_baselines(
    X_train: np.ndarray,
    X_test:  np.ndarray,
    feature_cols: List[str],
) -> Dict[str, np.ndarray]:
    n = len(X_test)
    return {
        "always_nomove"   : always_nomove(n),
        "always_move"     : always_move(n),
        "vol_threshold"   : vol_threshold_baseline(X_train, X_test, feature_cols),
    }


def run_task_b_baselines(
    X_test: np.ndarray,
    feature_cols: List[str],
) -> Dict[str, np.ndarray]:
    return {
        "always_up"        : always_up(len(X_test)),
        "always_down"      : always_down(len(X_test)),
        "prev_return_sign" : prev_return_sign(X_test, feature_cols),
        "rolling_momentum" : rolling_momentum_sign(X_test, feature_cols),
    }
