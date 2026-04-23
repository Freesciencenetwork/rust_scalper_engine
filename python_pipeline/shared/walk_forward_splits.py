"""
walk_forward_splits.py — Expanding-window walk-forward cross-validation.

Why walk-forward?
  Financial time series must NOT be randomly split.  The model must always
  be trained only on data that was historically available at the time of
  prediction.  Walk-forward is the standard protocol for this.

Structure (expanding window, n_folds=5)
  Given n total samples, divided into (n_folds + 1) equal chunks:

    chunk_size = n // (n_folds + 1)

    fold 0:  train=[0 : chunk*(1-val)]  val=[chunk*(1-val) : chunk]  test=[chunk : 2*chunk]
    fold 1:  train=[0 : 2*chunk*(1-val)]  val=[...  : 2*chunk]       test=[2*chunk : 3*chunk]
    ...
    fold 4:  train=[0 : 5*chunk*(1-val)]  val=[...  : 5*chunk]       test=[5*chunk : n]

  The training window grows with each fold (expanding window).
  Validation is carved from the tail of the training window to provide
  a clean early-stopping set without touching the test slice.

Notes
  - There is a deliberate gap of 0 bars between val end and test start
    because the targets already include a forward-look gap (horizon h).
    Adding an extra gap would waste data unnecessarily.
  - For the first fold, the training set may be small. Ensure MIN_FOLD_TRAIN
    is set appropriately (config.WF_N_FOLDS should be tuned so chunk is large
    enough to produce a meaningful model).
"""
from typing import Dict, List, Optional, Tuple

import logging
from typing import Generator

logger = logging.getLogger(__name__)


def expanding_window_splits(
    n: int,
    n_folds: int,
    val_ratio: float = 0.15,
) -> List[dict]:
    """
    Compute index ranges for an expanding walk-forward split.

    Parameters
    ----------
    n : int
        Total number of samples (already cleaned, no NaNs).
    n_folds : int
        Number of test folds.  Typical: 5.
    val_ratio : float
        Fraction of each train window reserved for validation (early stopping).

    Returns
    -------
    list of dicts, one per fold:
        {
          "fold"  : int,
          "train" : (start, end),   # exclusive end
          "val"   : (start, end),
          "test"  : (start, end),
        }
    """
    chunk = n // (n_folds + 1)

    if chunk < 100:
        raise ValueError(
            f"Dataset too small for {n_folds} folds: chunk_size={chunk}. "
            "Reduce n_folds or increase dataset size."
        )

    splits = []
    for i in range(n_folds):
        train_val_end = chunk * (i + 1)
        val_size      = max(1, int(train_val_end * val_ratio))
        train_end     = train_val_end - val_size

        test_start = train_val_end
        test_end   = chunk * (i + 2) if i < n_folds - 1 else n

        splits.append({
            "fold" : i,
            "train": (0, train_end),
            "val"  : (train_end, train_val_end),
            "test" : (test_start, test_end),
        })

        logger.debug(
            "Fold %d  train=[0:%d]  val=[%d:%d]  test=[%d:%d]",
            i, train_end, train_end, train_val_end, test_start, test_end,
        )

    # Print summary table
    logger.info("Walk-forward split plan (%d folds, val_ratio=%.2f):", n_folds, val_ratio)
    logger.info("  %-6s  %-12s  %-12s  %-12s", "Fold", "Train", "Val", "Test")
    for s in splits:
        tr = s["train"][1] - s["train"][0]
        vl = s["val"][1]   - s["val"][0]
        te = s["test"][1]  - s["test"][0]
        logger.info("  %-6d  %-12d  %-12d  %-12d", s["fold"], tr, vl, te)

    return splits


def slice_fold(df, split: dict):
    """
    Helper: slice a DataFrame using a split dict returned by expanding_window_splits.

    Returns (train_df, val_df, test_df).
    """
    tr_s, tr_e = split["train"]
    vl_s, vl_e = split["val"]
    te_s, te_e = split["test"]
    return (
        df.iloc[tr_s:tr_e].reset_index(drop=True),
        df.iloc[vl_s:vl_e].reset_index(drop=True),
        df.iloc[te_s:te_e].reset_index(drop=True),
    )
