"""
train_task_a.py — Task A: MOVE vs NO_MOVE on 1-minute BTC candles.

Objective
  Learn to distinguish bars where the next H minutes will produce a move
  larger than K * rolling_vol from bars where price stays roughly flat.

Use case
  A reliable MOVE detector acts as a trade-gating filter: only enter a
  position when the model says a significant move is expected.  This reduces
  overtrading and improves signal-to-noise even if the direction is unknown.

Signal test
  The model must beat the following baselines to claim signal:
    - always_nomove     (trivially optimal when MOVE is rare)
    - always_move
    - vol_threshold     (simple high-vol regime heuristic)

  Primary metric: MCC.  A model with MCC < 0.10 that fails to beat all
  baselines is reported as "No reliable predictive edge demonstrated."

Usage
  python train_task_a.py --data /path/to/btcusd_1-min_data.csv
  python train_task_a.py --data ... --horizon 5 --k 1.0 --vol-win 60 --n-folds 5
  python train_task_a.py --data ... --max-rows 500000   # fast dev run
"""
from typing import Dict, List, Optional, Tuple

import argparse
import json
import logging
import os
import sys

import numpy as np
import pandas as pd

import config
from data_loader import load_ohlcv
from features import add_features, add_extended_features, ALL_FEATURE_COLUMNS
from targets import make_vol_scaled_targets
from baselines import run_task_a_baselines
from metrics import binary_report, compare_model_vs_baselines, aggregate_fold_metrics
from walk_forward import expanding_window_splits, slice_fold

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(name)s — %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)


def _legacy_entrypoint_disabled() -> None:
    raise SystemExit(
        "train_task_a.py is disabled. This legacy path computes Python-side features.\n"
        "Use the Rust-backed pipeline instead:\n"
        "  cd python_pipeline\n"
        "  python3 prepare_rust_features.py --server http://127.0.0.1:8080\n"
        "  python3 train_v2.py --task a"
    )


def parse_args():
    p = argparse.ArgumentParser(description="Task A: MOVE vs NO_MOVE classifier")
    p.add_argument("--data",     default=config.DATA_PATH)
    p.add_argument("--horizon",  type=int,   default=config.TASK_A_HORIZON)
    p.add_argument("--k",        type=float, default=config.TASK_A_K)
    p.add_argument("--vol-win",  type=int,   default=config.TASK_A_VOL_WIN)
    p.add_argument("--n-folds",  type=int,   default=config.WF_N_FOLDS)
    p.add_argument("--max-rows", type=int,   default=None,
                   help="Limit dataset to last N rows for fast iteration")
    return p.parse_args()


def fit_lgbm(X_train, y_train, X_val, y_val):
    from lightgbm import LGBMClassifier, early_stopping, log_evaluation
    params = dict(config.LGBM_BINARY_PARAMS)
    model = LGBMClassifier(**params)
    model.fit(
        X_train, y_train,
        eval_set=[(X_val, y_val)],
        callbacks=[
            early_stopping(config.LGBM_BINARY_EARLY_STOPPING, verbose=False),
            log_evaluation(period=-1),
        ],
    )
    return model


def run_fold(fold_info: dict, df: pd.DataFrame, feature_cols: List[str]) -> Tuple[dict, dict]:
    """Train and evaluate one walk-forward fold.  Returns (model_metrics, baseline_metrics)."""
    train_df, val_df, test_df = slice_fold(df, fold_info)

    X_train = train_df[feature_cols].values
    y_train = train_df["is_move"].values.astype(int)
    X_val   = val_df[feature_cols].values
    y_val   = val_df["is_move"].values.astype(int)
    X_test  = test_df[feature_cols].values
    y_test  = test_df["is_move"].values.astype(int)
    fr_test = test_df["future_return"].values

    fold_n = fold_info["fold"]
    logger.info(
        "Fold %d  |  train=%d  val=%d  test=%d  MOVE_rate_train=%.1f%%",
        fold_n,
        len(X_train), len(X_val), len(X_test),
        100 * y_train.mean(),
    )

    # ── Train model ──────────────────────────────────────────────────────────
    model = fit_lgbm(X_train, y_train, X_val, y_val)
    y_pred = model.predict(X_test)

    logger.info("  Best iteration: %d", model.best_iteration_)

    # ── Evaluate model ───────────────────────────────────────────────────────
    model_metrics = binary_report(
        f"LightGBM (fold {fold_n})",
        y_test, y_pred, fr_test,
        pos_label=1,
        class_names=["NO_MOVE", "MOVE"],
        verbose=True,
    )

    # ── Evaluate baselines ───────────────────────────────────────────────────
    bl_preds = run_task_a_baselines(X_train, X_test, feature_cols)
    baseline_metrics = {}
    for bl_name, bl_pred in bl_preds.items():
        baseline_metrics[bl_name] = binary_report(
            bl_name, y_test, bl_pred, fr_test,
            pos_label=1,
            class_names=["NO_MOVE", "MOVE"],
            verbose=False,
        )

    compare_model_vs_baselines(model_metrics, baseline_metrics)
    return model_metrics, baseline_metrics


def main():
    args = parse_args()

    logger.info("═" * 60)
    logger.info("Task A: MOVE vs NO_MOVE — 1-minute BTC")
    logger.info("Horizon  : %d bars", args.horizon)
    logger.info("K        : %.2f × rolling_vol", args.k)
    logger.info("Vol win  : %d bars", args.vol_win)
    logger.info("Folds    : %d", args.n_folds)
    logger.info("Data     : %s", args.data)
    logger.info("═" * 60)

    # ── Load & build features ────────────────────────────────────────────────
    raw = load_ohlcv(args.data)

    if args.max_rows:
        raw = raw.tail(args.max_rows).reset_index(drop=True)
        logger.info("Dataset limited to last %d rows.", args.max_rows)

    df = add_features(raw)
    df = add_extended_features(df)

    # ── Build targets ────────────────────────────────────────────────────────
    df = make_vol_scaled_targets(
        df,
        horizon=args.horizon,
        k=args.k,
        vol_win=args.vol_win,
    )

    # ── Walk-forward splits ──────────────────────────────────────────────────
    splits = expanding_window_splits(len(df), n_folds=args.n_folds, val_ratio=config.WF_VAL_RATIO)

    # ── Run folds ────────────────────────────────────────────────────────────
    all_model_metrics    = []
    all_baseline_metrics = {}   # baseline_name -> list of per-fold metrics

    for fold_info in splits:
        m_metrics, b_metrics = run_fold(fold_info, df, ALL_FEATURE_COLUMNS)
        all_model_metrics.append(m_metrics)
        for bname, bmet in b_metrics.items():
            all_baseline_metrics.setdefault(bname, []).append(bmet)

    # ── Aggregate across folds ────────────────────────────────────────────────
    logger.info("\nModel walk-forward aggregate:")
    model_agg = aggregate_fold_metrics(all_model_metrics)

    logger.info("\nBaseline aggregates:")
    for bname, blist in all_baseline_metrics.items():
        logger.info("  %s  MCC_mean=%.4f", bname, np.mean([m["mcc"] for m in blist]))

    # ── Final verdict ────────────────────────────────────────────────────────
    best_model_mcc    = model_agg["mcc_mean"]
    best_baseline_mcc = max(
        np.mean([m["mcc"] for m in blist]) for blist in all_baseline_metrics.values()
    )
    print("\n" + "=" * 60)
    print("  FINAL VERDICT — Task A (MOVE vs NO_MOVE, 1m)")
    print("=" * 60)
    print(f"  Model MCC (mean over folds) : {best_model_mcc:+.4f}")
    print(f"  Best baseline MCC           : {best_baseline_mcc:+.4f}")
    if best_model_mcc > best_baseline_mcc + 0.02 and best_model_mcc > 0.10:
        print("  → USEFUL SIGNAL DETECTED: model is a meaningful no-trade filter.")
    elif best_model_mcc > best_baseline_mcc:
        print("  → MARGINAL EDGE: model beats baselines but signal is weak (MCC < 0.10).")
    else:
        print("  → NO RELIABLE PREDICTIVE EDGE DEMONSTRATED at this horizon/k/vol_win.")
        print("     Try adjusting --horizon or --k before adding more features.")
    print("=" * 60)

    # ── Save the model trained on the most recent (largest) fold ────────────
    # The last fold uses the most data and is the most representative.
    logger.info("Saving final model (trained on full data via last fold) ...")
    os.makedirs(config.MODELS_DIR, exist_ok=True)
    final_split  = splits[-1]
    # retrain on all data up to test_start for final model
    full_train = df.iloc[:final_split["test"][0]].reset_index(drop=True)
    val_size   = int(len(full_train) * config.WF_VAL_RATIO)
    X_ft = full_train[ALL_FEATURE_COLUMNS].values
    y_ft = full_train["is_move"].values.astype(int)
    X_fv = X_ft[-val_size:]
    y_fv = y_ft[-val_size:]
    X_ft = X_ft[:-val_size]
    y_ft = y_ft[:-val_size]

    final_model = fit_lgbm(X_ft, y_ft, X_fv, y_fv)

    model_path = os.path.join(config.MODELS_DIR, "task_a_lgbm.txt")
    final_model.booster_.save_model(model_path)
    logger.info("Saved Task A model -> %s", model_path)

    schema = {
        "task"            : "A",
        "description"     : "MOVE vs NO_MOVE",
        "feature_columns" : ALL_FEATURE_COLUMNS,
        "horizon"         : args.horizon,
        "k"               : args.k,
        "vol_win"         : args.vol_win,
        "label_move"      : config.TASK_A_LABEL_MOVE,
        "label_nomove"    : config.TASK_A_LABEL_NOMOVE,
        "model_mcc_mean"  : best_model_mcc,
        "model_path"      : model_path,
    }
    schema_path = os.path.join(config.MODELS_DIR, "task_a_schema.json")
    with open(schema_path, "w") as fh:
        json.dump(schema, fh, indent=2)
    logger.info("Saved Task A schema -> %s", schema_path)


if __name__ == "__main__":
    _legacy_entrypoint_disabled()
