"""
evaluate.py — Model evaluation utilities.

All functions are pure: they receive predictions/labels and return
dictionaries or print results.  No model loading or training here.
"""

import logging
import numpy as np
import pandas as pd
from sklearn.metrics import (
    accuracy_score,
    classification_report,
    confusion_matrix,
)
import config

logger = logging.getLogger(__name__)

_CLASS_NAMES = ["DOWN", "FLAT", "UP"]


def evaluate_model(
    model_name: str,
    y_true: np.ndarray,
    y_pred: np.ndarray,
    future_returns: np.ndarray,
) -> dict:
    """
    Print a full evaluation report for one model and return a summary dict.

    Parameters
    ----------
    model_name : str
        Display name used in headers (e.g. "XGBoost").
    y_true : np.ndarray, shape (n,)
        Ground-truth class labels (0 / 1 / 2).
    y_pred : np.ndarray, shape (n,)
        Predicted class labels.
    future_returns : np.ndarray, shape (n,)
        Raw future returns for each test bar (used for financial sanity-check).

    Returns
    -------
    dict
        Keys: accuracy, precision_up, precision_down,
              class_report (str), confusion (np.ndarray).
    """
    print("\n" + "=" * 60)
    print(f"  {model_name} — Test Set Evaluation")
    print("=" * 60)

    # ── Accuracy ─────────────────────────────────────────────────────────────
    acc = accuracy_score(y_true, y_pred)
    print(f"\nAccuracy : {acc:.4f}")

    # ── Classification report ────────────────────────────────────────────────
    report = classification_report(
        y_true, y_pred,
        target_names=_CLASS_NAMES,
        digits=4,
        zero_division=0,
    )
    print("\nClassification Report:")
    print(report)

    # ── Confusion matrix ─────────────────────────────────────────────────────
    cm = confusion_matrix(y_true, y_pred, labels=[0, 1, 2])
    print("Confusion Matrix (rows=true, cols=predicted):")
    print(_format_confusion_matrix(cm))

    # ── Prediction distribution ───────────────────────────────────────────────
    unique, counts = np.unique(y_pred, return_counts=True)
    dist = dict(zip(unique, counts))
    total = len(y_pred)
    print("\nPrediction Distribution:")
    for cls, name in enumerate(_CLASS_NAMES):
        n = dist.get(cls, 0)
        print(f"  {name:6s} ({cls}): {n:6d}  ({100*n/total:.1f}%)")

    # ── Average future return by predicted class ──────────────────────────────
    # This is a financial sanity-check: if the UP class truly carries positive
    # forward returns on average (and DOWN negative), the model has signal.
    print("\nAverage Future Return by Predicted Class:")
    for cls, name in enumerate(_CLASS_NAMES):
        mask = y_pred == cls
        if mask.sum() == 0:
            print(f"  {name:6s}: no predictions")
            continue
        avg_ret = future_returns[mask].mean()
        print(f"  {name:6s}: {avg_ret:+.6f}  (n={mask.sum()})")

    # ── Per-class precision (UP and DOWN are the actionable classes) ──────────
    from sklearn.metrics import precision_score
    prec_per_class = precision_score(
        y_true, y_pred,
        labels=[0, 1, 2],
        average=None,
        zero_division=0,
    )
    prec_up   = prec_per_class[config.CLASS_UP]
    prec_down = prec_per_class[config.CLASS_DOWN]
    print(f"\nPrecision — UP  : {prec_up:.4f}")
    print(f"Precision — DOWN: {prec_down:.4f}")

    return {
        "accuracy"      : float(acc),
        "precision_up"  : float(prec_up),
        "precision_down": float(prec_down),
        "class_report"  : report,
        "confusion"     : cm,
    }


def compare_and_select(
    xgb_metrics: dict,
    lgbm_metrics: dict,
) -> str:
    """
    Pick the better model based on test-set accuracy.

    Accuracy is used as the primary selection criterion here.  For
    production use you would substitute a risk-adjusted metric (e.g. Sharpe
    on a paper-trade back-test), but accuracy is the most robust default
    when the class distribution is reasonably balanced.

    Returns
    -------
    str
        "xgboost" or "lightgbm"
    """
    xgb_acc  = xgb_metrics["accuracy"]
    lgbm_acc = lgbm_metrics["accuracy"]

    print("\n" + "=" * 60)
    print("  Model Comparison")
    print("=" * 60)
    print(f"  XGBoost  accuracy : {xgb_acc:.4f}")
    print(f"  LightGBM accuracy : {lgbm_acc:.4f}")

    if xgb_acc >= lgbm_acc:
        winner = "xgboost"
    else:
        winner = "lightgbm"

    print(f"\n  Winner: {winner.upper()}")
    print("=" * 60)
    return winner


# ────────────────────────────────────────────────────────────────────────────
# Internal helpers
# ────────────────────────────────────────────────────────────────────────────

def _format_confusion_matrix(cm: np.ndarray) -> str:
    """Pretty-print a confusion matrix with row/col labels."""
    header = "         " + "  ".join(f"Pred {n:>4}" for n in _CLASS_NAMES)
    rows   = [header]
    for i, name in enumerate(_CLASS_NAMES):
        row = f"True {name:>4}  " + "  ".join(f"{cm[i,j]:9d}" for j in range(3))
        rows.append(row)
    return "\n".join(rows)
