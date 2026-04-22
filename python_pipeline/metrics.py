"""
metrics.py — Honest, imbalance-aware evaluation for binary tasks.

Raw accuracy is suppressed as the headline metric.  The primary metrics are:
  - Balanced accuracy   (equal weight per class)
  - MCC                 (Matthews Correlation Coefficient — single robust scalar)
  - F1 of positive class
  - Precision / Recall per class

Additional financial sanity checks:
  - Average future return by predicted class
    (UP predictions should carry positive expected return, DOWN negative)
  - For Task A: whether the model is a useful no-trade filter
    (NO_MOVE predictions should have near-zero expected absolute return)

Verdict thresholds
------------------
  MCC > 0.10  = weak but real signal
  MCC > 0.20  = usable signal
  MCC > 0.35  = strong signal
"""
from typing import Dict, List, Optional, Tuple

import numpy as np
from sklearn.metrics import (
    accuracy_score,
    balanced_accuracy_score,
    classification_report,
    confusion_matrix,
    f1_score,
    matthews_corrcoef,
    precision_score,
    recall_score,
)


def binary_report(
    name: str,
    y_true: np.ndarray,
    y_pred: np.ndarray,
    future_returns: np.ndarray,
    pos_label: int = 1,
    class_names: Optional[List[str]] = None,
    verbose: bool = True,
) -> dict:
    """
    Full binary evaluation.  Prints a report and returns a metrics dict.

    Parameters
    ----------
    name : str
        Display label for the predictor (model name or baseline name).
    y_true : np.ndarray  shape (n,)
    y_pred : np.ndarray  shape (n,)
    future_returns : np.ndarray  shape (n,)
        Raw future return for each test bar.
    pos_label : int
        Which class is the "positive" class (1 = MOVE or UP).
    class_names : List[str]
        e.g. ["NO_MOVE", "MOVE"] or ["DOWN", "UP"]
    verbose : bool
        If True, print the full report.

    Returns
    -------
    dict with keys: accuracy, balanced_accuracy, precision, recall,
                    f1, mcc, confusion, avg_ret_by_class, prediction_dist
    """
    if class_names is None:
        class_names = [str(i) for i in sorted(set(y_true))]

    labels = list(range(len(class_names)))

    acc      = accuracy_score(y_true, y_pred)
    bal_acc  = balanced_accuracy_score(y_true, y_pred)
    mcc      = matthews_corrcoef(y_true, y_pred)
    cm       = confusion_matrix(y_true, y_pred, labels=labels)

    prec_per = precision_score(y_true, y_pred, labels=labels, average=None, zero_division=0)
    rec_per  = recall_score(y_true, y_pred,    labels=labels, average=None, zero_division=0)
    f1_per   = f1_score(y_true, y_pred,        labels=labels, average=None, zero_division=0)

    prec_pos = float(prec_per[pos_label]) if pos_label < len(prec_per) else 0.0
    rec_pos  = float(rec_per[pos_label])  if pos_label < len(rec_per)  else 0.0
    f1_pos   = float(f1_per[pos_label])   if pos_label < len(f1_per)   else 0.0

    # Average future return by predicted class
    avg_ret = {}
    for i, cname in enumerate(class_names):
        mask = y_pred == i
        avg_ret[cname] = float(future_returns[mask].mean()) if mask.any() else float("nan")

    # Prediction distribution
    unique, counts = np.unique(y_pred, return_counts=True)
    pred_dist = {class_names[i]: int(counts[j]) for j, i in enumerate(unique) if i < len(class_names)}

    if verbose:
        sep = "─" * 56
        print(f"\n{sep}")
        print(f"  {name}")
        print(sep)
        print(f"  Accuracy          : {acc:.4f}")
        print(f"  Balanced Accuracy : {bal_acc:.4f}  ← primary metric")
        print(f"  MCC               : {mcc:+.4f}  ← single scalar signal test")
        print(f"  F1 (pos class)    : {f1_pos:.4f}")
        print(f"  Precision (pos)   : {prec_pos:.4f}")
        print(f"  Recall (pos)      : {rec_pos:.4f}")
        print()

        # Per-class breakdown
        print("  Per-class metrics:")
        for i, cname in enumerate(class_names):
            if i < len(prec_per):
                print(f"    {cname:>10}  prec={prec_per[i]:.4f}  rec={rec_per[i]:.4f}  f1={f1_per[i]:.4f}")

        # Confusion matrix
        print()
        print("  Confusion matrix (rows=true, cols=predicted):")
        header = "            " + "  ".join(f"P:{c:>7}" for c in class_names)
        print(f"  {header}")
        for i, cname in enumerate(class_names):
            row = "  ".join(f"{cm[i, j]:9d}" for j in range(len(class_names)))
            print(f"  T:{cname:>8}  {row}")

        # Prediction distribution
        print()
        total = len(y_pred)
        print("  Prediction distribution:")
        for cname, cnt in pred_dist.items():
            print(f"    {cname:>10} : {cnt:7d}  ({100*cnt/total:.1f}%)")

        # Average future return by predicted class
        print()
        print("  Avg future return by predicted class:")
        for cname, ret in avg_ret.items():
            print(f"    {cname:>10} : {ret:+.6f}")

    return {
        "name"             : name,
        "accuracy"         : float(acc),
        "balanced_accuracy": float(bal_acc),
        "mcc"              : float(mcc),
        "f1_pos"           : float(f1_pos),
        "precision_pos"    : prec_pos,
        "recall_pos"       : rec_pos,
        "prec_per_class"   : prec_per.tolist(),
        "rec_per_class"    : rec_per.tolist(),
        "f1_per_class"     : f1_per.tolist(),
        "confusion"        : cm,
        "avg_ret_by_class" : avg_ret,
        "prediction_dist"  : pred_dist,
    }


def compare_model_vs_baselines(
    model_metrics: dict,
    baseline_metrics: Dict[str, dict],
    metric_key: str = "mcc",
) -> str:
    """
    Print a comparison table and return a verdict string.

    Parameters
    ----------
    model_metrics : dict    output of binary_report() for the ML model
    baseline_metrics : dict  {baseline_name: binary_report_dict}
    metric_key : str         which metric to rank by (default: mcc)

    Returns
    -------
    "signal"    if model beats all baselines by a meaningful margin (MCC > 0.10)
    "marginal"  if model beats baselines but MCC < 0.10
    "no_edge"   if model fails to beat the best baseline
    """
    print("\n" + "=" * 56)
    print("  Model vs Baselines")
    print("=" * 56)

    rows = [(model_metrics["name"], model_metrics[metric_key], True)]
    for bname, bmet in baseline_metrics.items():
        rows.append((bname, bmet[metric_key], False))

    rows.sort(key=lambda x: x[1], reverse=True)

    for name, val, is_model in rows:
        tag = " ← MODEL" if is_model else ""
        print(f"  {name:28s}  {metric_key.upper()}={val:+.4f}{tag}")

    model_val    = model_metrics[metric_key]
    best_baseline = max(bmet[metric_key] for bmet in baseline_metrics.values())

    print()
    if model_val > best_baseline + 0.02 and model_val > 0.10:
        verdict = "signal"
        print(f"  VERDICT: USEFUL SIGNAL DETECTED  (model {metric_key.upper()}={model_val:+.4f})")
    elif model_val > best_baseline:
        verdict = "marginal"
        print(f"  VERDICT: MARGINAL EDGE  (model beats baselines but {metric_key.upper()}={model_val:+.4f} < 0.10)")
    else:
        verdict = "no_edge"
        print(
            f"  VERDICT: NO RELIABLE EDGE DEMONSTRATED\n"
            f"  No reliable predictive edge was demonstrated at this horizon\n"
            f"  with the current data and features."
        )
    print("=" * 56)
    return verdict


def aggregate_fold_metrics(fold_metrics: List[dict]) -> dict:
    """
    Average scalar metrics across walk-forward folds and print a summary.

    Parameters
    ----------
    fold_metrics : list of binary_report() dicts, one per fold.

    Returns
    -------
    dict with mean and std of each scalar metric.
    """
    scalar_keys = ["accuracy", "balanced_accuracy", "mcc", "f1_pos",
                   "precision_pos", "recall_pos"]

    summary = {}
    print("\n" + "=" * 56)
    print("  Walk-Forward Summary  (mean ± std across folds)")
    print("=" * 56)
    for key in scalar_keys:
        vals = [m[key] for m in fold_metrics if key in m]
        mean = float(np.mean(vals))
        std  = float(np.std(vals))
        summary[f"{key}_mean"] = mean
        summary[f"{key}_std"]  = std
        print(f"  {key:22s}: {mean:+.4f} ± {std:.4f}")
    print("=" * 56)
    return summary
