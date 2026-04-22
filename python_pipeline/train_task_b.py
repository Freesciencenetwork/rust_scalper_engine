"""
train_task_b.py — Task B: UP vs DOWN on MOVE bars (15-minute candles).

Objective
  Given that we already know a significant move is occurring (either via
  Task A or because we are looking at a MOVE bar after the fact), predict
  whether the move is upward or downward.

Input
  1-minute raw CSV, resampled to 15-minute OHLCV internally.
  Training and evaluation use only bars labelled as MOVE by the
  vol-scaled target formula.

Why 15-minute?
  At 1-minute resolution, noise dominates direction prediction.
  15-minute bars smooth out micro-noise while retaining intraday structure.
  The 1m data is used as source to maximize the available history.

Signal test
  The model must beat:
    - always_up           (naive bullish bias)
    - always_down
    - prev_return_sign    (momentum continuation)
    - rolling_momentum    (medium-term trend)

  Primary metric: MCC.

Usage
  python train_task_b.py --data /path/to/btcusd_1-min_data.csv
  python train_task_b.py --data ... --horizon 3 --k 1.0 --vol-win 30
  python train_task_b.py --data ... --max-rows 500000 --n-folds 3
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
from data_loader import load_ohlcv, resample_ohlcv
from features import add_features, add_extended_features, ALL_FEATURE_COLUMNS
from targets import make_vol_scaled_targets
from baselines import run_task_b_baselines
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
        "train_task_b.py is disabled. This legacy path computes Python-side features.\n"
        "Use the Rust-backed pipeline instead:\n"
        "  cd python_pipeline\n"
        "  python3 prepare_rust_features.py --server http://127.0.0.1:8080\n"
        "  python3 train_v2.py --task b"
    )


def parse_args():
    p = argparse.ArgumentParser(description="Task B: UP vs DOWN on MOVE bars (15m candles)")
    p.add_argument("--data",      default=config.DATA_PATH)
    p.add_argument("--resample",  default="15min",
                   help="pandas offset alias for resampling (default: 15min)")
    p.add_argument("--horizon",   type=int,   default=config.TASK_B_HORIZON)
    p.add_argument("--k",         type=float, default=config.TASK_B_K)
    p.add_argument("--vol-win",   type=int,   default=config.TASK_B_VOL_WIN)
    p.add_argument("--n-folds",   type=int,   default=config.WF_N_FOLDS)
    p.add_argument("--max-rows",  type=int,   default=None,
                   help="Limit 1m rows before resampling (for fast dev runs)")
    return p.parse_args()


def fit_lgbm(X_train, y_train, X_val, y_val):
    from lightgbm import LGBMClassifier, early_stopping, log_evaluation
    params = dict(config.LGBM_BINARY_PARAMS)
    params["is_unbalance"] = False   # Task B should be balanced after MOVE filter
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
    """
    Run one walk-forward fold for Task B.

    The split is done on the FULL dataset (including NO_MOVE bars) to maintain
    correct chronological ordering, then each split is filtered to MOVE bars only.
    This prevents the walk-forward boundary from shifting due to filtering.
    """
    train_df, val_df, test_df = slice_fold(df, fold_info)

    # Filter to MOVE bars only within each split
    train_move = train_df[train_df["is_move"] == 1].reset_index(drop=True)
    val_move   = val_df[val_df["is_move"] == 1].reset_index(drop=True)
    test_move  = test_df[test_df["is_move"] == 1].reset_index(drop=True)

    fold_n = fold_info["fold"]

    if len(train_move) < 50 or len(test_move) < 10:
        logger.warning("Fold %d: too few MOVE bars (train=%d, test=%d) — skipping.",
                       fold_n, len(train_move), len(test_move))
        return None, None

    X_train = train_move[feature_cols].values
    y_train = train_move["direction"].values.astype(int)
    X_val   = val_move[feature_cols].values
    y_val   = val_move["direction"].values.astype(int)
    X_test  = test_move[feature_cols].values
    y_test  = test_move["direction"].values.astype(int)
    fr_test = test_move["future_return"].values

    logger.info(
        "Fold %d  |  MOVE bars: train=%d  val=%d  test=%d  UP_rate_train=%.1f%%",
        fold_n, len(X_train), len(X_val), len(X_test),
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
        class_names=["DOWN", "UP"],
        verbose=True,
    )

    # ── Evaluate baselines ───────────────────────────────────────────────────
    bl_preds = run_task_b_baselines(X_test, feature_cols)
    baseline_metrics = {}
    for bl_name, bl_pred in bl_preds.items():
        baseline_metrics[bl_name] = binary_report(
            bl_name, y_test, bl_pred, fr_test,
            pos_label=1,
            class_names=["DOWN", "UP"],
            verbose=False,
        )

    compare_model_vs_baselines(model_metrics, baseline_metrics)
    return model_metrics, baseline_metrics


def main():
    args = parse_args()

    logger.info("═" * 60)
    logger.info("Task B: UP vs DOWN on MOVE bars — 15-minute BTC")
    logger.info("Resample : %s", args.resample)
    logger.info("Horizon  : %d bars", args.horizon)
    logger.info("K        : %.2f × rolling_vol", args.k)
    logger.info("Vol win  : %d bars", args.vol_win)
    logger.info("Folds    : %d", args.n_folds)
    logger.info("Data     : %s", args.data)
    logger.info("═" * 60)

    # ── Load 1m data ─────────────────────────────────────────────────────────
    raw_1m = load_ohlcv(args.data)

    if args.max_rows:
        raw_1m = raw_1m.tail(args.max_rows).reset_index(drop=True)
        logger.info("1m dataset limited to last %d rows.", args.max_rows)

    # ── Resample to 15m ──────────────────────────────────────────────────────
    raw = resample_ohlcv(raw_1m, rule=args.resample)

    # ── Feature engineering ───────────────────────────────────────────────────
    df = add_features(raw)
    df = add_extended_features(df)

    # ── Targets ───────────────────────────────────────────────────────────────
    df = make_vol_scaled_targets(
        df,
        horizon=args.horizon,
        k=args.k,
        vol_win=args.vol_win,
    )

    logger.info(
        "MOVE bars available for Task B: %d / %d (%.1f%%)",
        int((df["is_move"] == 1).sum()),
        len(df),
        100 * (df["is_move"] == 1).mean(),
    )

    # ── Walk-forward splits (on full chronological dataset) ──────────────────
    splits = expanding_window_splits(len(df), n_folds=args.n_folds, val_ratio=config.WF_VAL_RATIO)

    # ── Run folds ────────────────────────────────────────────────────────────
    all_model_metrics    = []
    all_baseline_metrics = {}

    for fold_info in splits:
        m_metrics, b_metrics = run_fold(fold_info, df, ALL_FEATURE_COLUMNS)
        if m_metrics is None:
            continue
        all_model_metrics.append(m_metrics)
        for bname, bmet in b_metrics.items():
            all_baseline_metrics.setdefault(bname, []).append(bmet)

    if not all_model_metrics:
        logger.error("No folds completed successfully. Try --k or --horizon adjustments.")
        sys.exit(1)

    # ── Aggregate ─────────────────────────────────────────────────────────────
    logger.info("\nModel walk-forward aggregate:")
    model_agg = aggregate_fold_metrics(all_model_metrics)

    logger.info("\nBaseline aggregates:")
    for bname, blist in all_baseline_metrics.items():
        logger.info("  %s  MCC_mean=%.4f", bname, np.mean([m["mcc"] for m in blist]))

    # ── Final verdict ─────────────────────────────────────────────────────────
    best_model_mcc    = model_agg["mcc_mean"]
    best_baseline_mcc = max(
        np.mean([m["mcc"] for m in blist]) for blist in all_baseline_metrics.values()
    )
    print("\n" + "=" * 60)
    print("  FINAL VERDICT — Task B (UP vs DOWN on MOVE bars, 15m)")
    print("=" * 60)
    print(f"  Model MCC (mean over folds) : {best_model_mcc:+.4f}")
    print(f"  Best baseline MCC           : {best_baseline_mcc:+.4f}")
    if best_model_mcc > best_baseline_mcc + 0.02 and best_model_mcc > 0.10:
        print("  → USEFUL SIGNAL DETECTED: direction prediction is meaningful.")
    elif best_model_mcc > best_baseline_mcc:
        print("  → MARGINAL EDGE: model beats baselines but MCC < 0.10.")
    else:
        print("  → NO RELIABLE PREDICTIVE EDGE DEMONSTRATED at this horizon/k/vol_win.")
        print("     Try --horizon 5 or --k 0.8 before adding more features.")
    print("=" * 60)

    # ── Save final model ──────────────────────────────────────────────────────
    os.makedirs(config.MODELS_DIR, exist_ok=True)
    final_split = splits[-1]
    full_train  = df.iloc[:final_split["test"][0]].reset_index(drop=True)
    full_move   = full_train[full_train["is_move"] == 1].reset_index(drop=True)
    val_size    = int(len(full_move) * config.WF_VAL_RATIO)

    X_ft = full_move[ALL_FEATURE_COLUMNS].values
    y_ft = full_move["direction"].values.astype(int)
    X_fv, y_fv = X_ft[-val_size:], y_ft[-val_size:]
    X_ft, y_ft = X_ft[:-val_size], y_ft[:-val_size]

    final_model = fit_lgbm(X_ft, y_ft, X_fv, y_fv)

    model_path = os.path.join(config.MODELS_DIR, "task_b_lgbm.txt")
    final_model.booster_.save_model(model_path)
    logger.info("Saved Task B model -> %s", model_path)

    schema = {
        "task"           : "B",
        "description"    : "UP vs DOWN on MOVE bars",
        "resample_rule"  : args.resample,
        "feature_columns": ALL_FEATURE_COLUMNS,
        "horizon"        : args.horizon,
        "k"              : args.k,
        "vol_win"        : args.vol_win,
        "label_up"       : config.TASK_B_LABEL_UP,
        "label_down"     : config.TASK_B_LABEL_DOWN,
        "model_mcc_mean" : best_model_mcc,
        "model_path"     : model_path,
    }
    schema_path = os.path.join(config.MODELS_DIR, "task_b_schema.json")
    with open(schema_path, "w") as fh:
        json.dump(schema, fh, indent=2)
    logger.info("Saved Task B schema -> %s", schema_path)


if __name__ == "__main__":
    _legacy_entrypoint_disabled()
