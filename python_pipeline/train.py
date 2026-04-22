"""
train.py — End-to-end training pipeline.

Pipeline:
  raw CSV -> data_loader -> features -> target -> time split
         -> XGBoost train -> evaluate
         -> LightGBM train -> evaluate
         -> compare -> save best model

Usage:
  python train.py [--horizon H] [--threshold T] [--data PATH]

Arguments:
  --horizon    : bars ahead for future return (default: config.HORIZON)
  --threshold  : min move to label as UP/DOWN  (default: config.THRESHOLD)
  --data       : path to OHLCV CSV             (default: config.DATA_PATH)
"""

import argparse
import json
import logging
import os
import sys

import numpy as np

import config
from data_loader import load_ohlcv
from features import add_features, add_target, FEATURE_COLUMNS
from evaluate import evaluate_model, compare_and_select

# ────────────────────────────────────────────────────────────────────────────
# Logging — configure once here so every module that does logging.getLogger()
# automatically inherits the level and handler.
# ────────────────────────────────────────────────────────────────────────────
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(name)s — %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)


def _legacy_entrypoint_disabled() -> None:
    raise SystemExit(
        "train.py is disabled. This legacy path computes Python-side features.\n"
        "Use the Rust-backed pipeline instead:\n"
        "  cd python_pipeline\n"
        "  python3 prepare_rust_features.py --server http://127.0.0.1:8080\n"
        "  python3 train_v2.py --task both"
    )


# ────────────────────────────────────────────────────────────────────────────
# Lazy imports — XGBoost and LightGBM are only imported after argument parsing
# so that --help works even without them installed.
# ────────────────────────────────────────────────────────────────────────────

def _import_models():
    try:
        import xgboost as xgb
        import lightgbm as lgb
    except ImportError as e:
        logger.error("Missing dependency: %s", e)
        sys.exit(1)
    return xgb, lgb


# ────────────────────────────────────────────────────────────────────────────
# Time-based split
# ────────────────────────────────────────────────────────────────────────────

def time_split(df, train_ratio=config.TRAIN_RATIO, val_ratio=config.VAL_RATIO):
    """
    Chronological split — NO shuffling.

    Train on the past, validate on more recent data, test on the most recent.
    This mimics real deployment where the model always sees future data at
    inference time.
    """
    n        = len(df)
    n_train  = int(n * train_ratio)
    n_val    = int(n * val_ratio)

    train_df = df.iloc[:n_train].copy()
    val_df   = df.iloc[n_train : n_train + n_val].copy()
    test_df  = df.iloc[n_train + n_val :].copy()

    logger.info(
        "Split sizes — train: %d  val: %d  test: %d",
        len(train_df), len(val_df), len(test_df),
    )
    return train_df, val_df, test_df


def extract_xy(df):
    """Return feature matrix X, label vector y, and raw future returns."""
    X  = df[FEATURE_COLUMNS].values
    y  = df["target"].values.astype(int)
    fr = df["future_return"].values
    return X, y, fr


# ────────────────────────────────────────────────────────────────────────────
# Training functions
# ────────────────────────────────────────────────────────────────────────────

def train_xgboost(X_train, y_train, X_val, y_val, xgb):
    """
    Fit an XGBoost multiclass classifier with early stopping on validation
    log-loss.  Returns the fitted model.
    """
    from xgboost import XGBClassifier

    params = dict(config.XGB_PARAMS)  # shallow copy — don't mutate config
    early_stopping_rounds = params.pop("early_stopping_rounds")

    model = XGBClassifier(
        **params,
        early_stopping_rounds=early_stopping_rounds,
    )

    logger.info("Training XGBoost  (n_estimators=%d, early_stopping=%d) …",
                params["n_estimators"], early_stopping_rounds)

    model.fit(
        X_train, y_train,
        eval_set=[(X_val, y_val)],
        verbose=False,
    )

    best = model.best_iteration
    logger.info("XGBoost best iteration: %d", best)
    return model


def train_lightgbm(X_train, y_train, X_val, y_val, lgb):
    """
    Fit a LightGBM multiclass classifier with early stopping.
    Returns the fitted model.
    """
    from lightgbm import LGBMClassifier, early_stopping, log_evaluation

    params = dict(config.LGBM_PARAMS)

    model = LGBMClassifier(**params)

    logger.info("Training LightGBM (n_estimators=%d, early_stopping=%d) …",
                params["n_estimators"], config.LGBM_EARLY_STOPPING_ROUNDS)

    model.fit(
        X_train, y_train,
        eval_set=[(X_val, y_val)],
        callbacks=[
            early_stopping(config.LGBM_EARLY_STOPPING_ROUNDS, verbose=False),
            log_evaluation(period=-1),   # silence per-iteration output
        ],
    )

    best = model.best_iteration_
    logger.info("LightGBM best iteration: %d", best)
    return model


# ────────────────────────────────────────────────────────────────────────────
# Persistence
# ────────────────────────────────────────────────────────────────────────────

def save_models(xgb_model, lgbm_model, winner: str, horizon: int, threshold: float):
    """Save both models and a JSON schema file for inference."""
    os.makedirs(config.MODELS_DIR, exist_ok=True)

    # XGBoost — native JSON format is portable and version-stable
    xgb_model.save_model(config.XGB_MODEL_PATH)
    logger.info("Saved XGBoost model  -> %s", config.XGB_MODEL_PATH)

    # LightGBM — .txt is the native text format
    lgbm_model.booster_.save_model(config.LGBM_MODEL_PATH)
    logger.info("Saved LightGBM model -> %s", config.LGBM_MODEL_PATH)

    # Feature schema — used by inference code to guarantee identical column
    # ordering and to detect feature-set drift before it causes silent errors.
    schema = {
        "feature_columns" : FEATURE_COLUMNS,
        "num_features"    : len(FEATURE_COLUMNS),
        "num_classes"     : config.NUM_CLASSES,
        "class_labels"    : {
            str(config.CLASS_DOWN): "DOWN",
            str(config.CLASS_FLAT): "FLAT",
            str(config.CLASS_UP)  : "UP",
        },
        "horizon"         : horizon,
        "threshold"       : threshold,
        "best_model"      : winner,
        "xgb_model_path"  : config.XGB_MODEL_PATH,
        "lgbm_model_path" : config.LGBM_MODEL_PATH,
    }

    with open(config.SCHEMA_PATH, "w") as fh:
        json.dump(schema, fh, indent=2)
    logger.info("Saved feature schema -> %s", config.SCHEMA_PATH)


# ────────────────────────────────────────────────────────────────────────────
# Main entry point
# ────────────────────────────────────────────────────────────────────────────

def parse_args():
    parser = argparse.ArgumentParser(
        description="Train XGBoost + LightGBM classifiers on BTC OHLCV data."
    )
    parser.add_argument(
        "--horizon",
        type=int,
        default=config.HORIZON,
        help=f"Forward return horizon in bars (default: {config.HORIZON})",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=config.THRESHOLD,
        help=f"Min move threshold for UP/DOWN label (default: {config.THRESHOLD})",
    )
    parser.add_argument(
        "--data",
        type=str,
        default=config.DATA_PATH,
        help=f"Path to OHLCV CSV (default: {config.DATA_PATH})",
    )
    return parser.parse_args()


def main():
    args = parse_args()
    xgb, lgb = _import_models()

    logger.info("═" * 60)
    logger.info("BTC Direction Prediction — Training Pipeline")
    logger.info("Horizon   : %d bars", args.horizon)
    logger.info("Threshold : %.4f (%.2f%%)", args.threshold, args.threshold * 100)
    logger.info("Data      : %s", args.data)
    logger.info("═" * 60)

    # ── 1. Load ──────────────────────────────────────────────────────────────
    raw_df = load_ohlcv(args.data)

    # ── 2. Feature engineering ───────────────────────────────────────────────
    feat_df = add_features(raw_df)

    # ── 3. Target creation ───────────────────────────────────────────────────
    full_df = add_target(feat_df, horizon=args.horizon, threshold=args.threshold)

    # ── 4. Time-based split ──────────────────────────────────────────────────
    train_df, val_df, test_df = time_split(full_df)

    X_train, y_train, _        = extract_xy(train_df)
    X_val,   y_val,   _        = extract_xy(val_df)
    X_test,  y_test,  fr_test  = extract_xy(test_df)

    # ── 5. Train XGBoost ─────────────────────────────────────────────────────
    xgb_model = train_xgboost(X_train, y_train, X_val, y_val, xgb)

    # ── 6. Train LightGBM ────────────────────────────────────────────────────
    lgbm_model = train_lightgbm(X_train, y_train, X_val, y_val, lgb)

    # ── 7. Evaluate on held-out test set ─────────────────────────────────────
    xgb_pred  = xgb_model.predict(X_test)
    lgbm_pred = lgbm_model.predict(X_test)

    xgb_metrics  = evaluate_model("XGBoost",  y_test, xgb_pred,  fr_test)
    lgbm_metrics = evaluate_model("LightGBM", y_test, lgbm_pred, fr_test)

    # ── 8. Select best model ─────────────────────────────────────────────────
    winner = compare_and_select(xgb_metrics, lgbm_metrics)

    # ── 9. Persist both models + schema ──────────────────────────────────────
    save_models(xgb_model, lgbm_model, winner, args.horizon, args.threshold)

    logger.info("Pipeline complete.  Best model: %s", winner.upper())


if __name__ == "__main__":
    _legacy_entrypoint_disabled()
