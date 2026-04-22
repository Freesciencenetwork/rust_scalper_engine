"""
train_v2.py — Walk-forward Task A + Task B using 129 Rust-computed, normalized features.

Pipeline
--------
  features_normalized.parquet  (129 features, timestamp_ms)
        +
  btcusd_1-min_data.csv        (close price for target computation)
        |
        v
  merge on timestamp_ms <-> unix_seconds * 1000
        |
        v
  vol-scaled targets  (is_move, direction)
        |
        +---- Task A: MOVE vs NO_MOVE  (1m, walk-forward, all rows)
        +---- Task B: UP vs DOWN       (15m resampled, walk-forward, MOVE rows only)

Usage
-----
  python3 train_v2.py                          # run both tasks, full data
  python3 train_v2.py --task a                 # Task A only
  python3 train_v2.py --task b                 # Task B only
  python3 train_v2.py --max-rows 300000        # fast dev run
"""

import argparse
import gc
import json
import logging
import os
import sys

os.environ.setdefault("MPLCONFIGDIR", "/tmp/matplotlib")

import numpy as np
import pandas as pd

import config
from data_loader import load_ohlcv, resample_ohlcv
from targets import make_vol_scaled_targets
from baselines import run_task_a_baselines, run_task_b_baselines
from metrics import binary_report, compare_model_vs_baselines, aggregate_fold_metrics
from walk_forward import expanding_window_splits, slice_fold
from normalize_features import feature_columns as get_feature_columns

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(name)s — %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)

FEATURES_PARQUET = "data/features_normalized.parquet"
OHLCV_CSV        = "/Users/francesco/Desktop/rust_scalper_engine/src/historical_data/btcusd_1-min_data.csv"


# ────────────────────────────────────────────────────────────────────────────
# Data assembly
# ────────────────────────────────────────────────────────────────────────────

def load_merged(ohlcv_path: str, features_path: str, max_rows) -> pd.DataFrame:
    """
    Merge OHLCV CSV with normalized Rust feature parquet on timestamp.

    Only 'close' is kept from OHLCV — it is needed solely to compute the
    future_return label (forward-looking by design, cannot come from Rust).
    All features come exclusively from the Rust-computed parquet.
    No Python indicators or formulas are added here.
    """
    logger.info("Loading OHLCV from %s ...", ohlcv_path)
    ohlcv_raw = pd.read_csv(ohlcv_path)
    ohlcv_raw.columns = ohlcv_raw.columns.str.lower()
    ohlcv_raw["close"] = pd.to_numeric(ohlcv_raw["close"], errors="coerce")
    ohlcv_raw.dropna(subset=["close"], inplace=True)
    ohlcv_raw["timestamp_ms"] = (ohlcv_raw["timestamp"].astype(float) * 1000).astype("int64")
    ohlcv_raw.sort_values("timestamp_ms", inplace=True)
    ohlcv_raw.reset_index(drop=True, inplace=True)
    logger.info("Loaded %d OHLCV rows", len(ohlcv_raw))

    logger.info("Loading normalized features from %s ...", features_path)
    feats = pd.read_parquet(features_path)

    logger.info("Merging (inner join on timestamp_ms) ...")
    df = pd.merge(ohlcv_raw[["timestamp_ms", "close"]], feats, on="timestamp_ms", how="inner")
    df.sort_values("timestamp_ms", inplace=True)
    df.reset_index(drop=True, inplace=True)

    logger.info("Merged shape: %s  (%s → %s)",
                df.shape,
                pd.to_datetime(df["timestamp_ms"].iloc[0],  unit="ms"),
                pd.to_datetime(df["timestamp_ms"].iloc[-1], unit="ms"))

    if max_rows:
        df = df.tail(max_rows).reset_index(drop=True)
        logger.info("Trimmed to last %d rows.", max_rows)

    return df


def validate_feature_provenance(features_path: str) -> dict:
    metadata_path = os.path.join(
        os.path.dirname(features_path),
        f"{os.path.splitext(os.path.basename(features_path))[0]}.metadata.json",
    )
    if not os.path.exists(metadata_path):
        raise FileNotFoundError(
            f"Missing feature metadata file: {metadata_path}. "
            "Build features with prepare_rust_features.py."
        )

    with open(metadata_path) as fh:
        metadata = json.load(fh)

    if metadata.get("source") != "rust_backend":
        raise ValueError(
            f"Feature file {features_path} is not marked as rust_backend-derived."
        )
    if metadata.get("pipeline_stage") != "normalized_features":
        raise ValueError(
            f"Feature file {features_path} has unexpected pipeline_stage="
            f"{metadata.get('pipeline_stage')!r}; expected 'normalized_features'."
        )
    return metadata


def resample_to_15m(df_1m: pd.DataFrame, feat_cols: list) -> pd.DataFrame:
    """
    Resample 1m merged DataFrame to 15m.
    OHLCV: standard aggregation. Features: last value in each 15m bucket.
    """
    df_1m = df_1m.copy()
    df_1m.index = pd.to_datetime(df_1m["timestamp_ms"], unit="ms", utc=True)

    ohlcv_agg = df_1m[["open", "high", "low", "close", "volume"]].resample("15min").agg(
        open=("open",   "first"),
        high=("high",   "max"),
        low=("low",     "min"),
        close=("close", "last"),
        volume=("volume", "sum"),
    )

    # For features: last value in the 15m window is the most recent signal
    feat_agg = df_1m[feat_cols].resample("15min").last()

    result = pd.concat([ohlcv_agg, feat_agg], axis=1).dropna(subset=["close"])
    result.index.name = "timestamp"
    result = result.reset_index()
    result["timestamp_ms"] = result["timestamp"].astype("int64") // 10**6
    result["ret_1"] = result["close"].pct_change(1)
    result.reset_index(drop=True, inplace=True)

    logger.info("Resampled 1m→15m: %d rows", len(result))
    return result


# ────────────────────────────────────────────────────────────────────────────
# Model
# ────────────────────────────────────────────────────────────────────────────

def fit_lgbm_binary(X_train, y_train, X_val, y_val, is_unbalance=True):
    from lightgbm import LGBMClassifier, early_stopping, log_evaluation
    params = dict(config.LGBM_BINARY_PARAMS)
    params["is_unbalance"] = is_unbalance
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


# ────────────────────────────────────────────────────────────────────────────
# Task A: MOVE vs NO_MOVE
# ────────────────────────────────────────────────────────────────────────────

def run_task_a(df: pd.DataFrame, feat_cols: list, n_folds: int):
    logger.info("\n%s\nTask A: MOVE vs NO_MOVE (1m)\n%s", "═"*60, "═"*60)

    df = make_vol_scaled_targets(
        df,
        horizon=config.TASK_A_HORIZON,
        k=config.TASK_A_K,
        vol_win=config.TASK_A_VOL_WIN,
        feature_cols=feat_cols,
    )

    splits = expanding_window_splits(len(df), n_folds=n_folds, val_ratio=config.WF_VAL_RATIO)
    all_model, all_base = [], {}

    for s in splits:
        train_df, val_df, test_df = slice_fold(df, s)

        X_tr = train_df[feat_cols];         y_tr = train_df["is_move"].values.astype(int)
        X_vl = val_df[feat_cols];           y_vl = val_df["is_move"].values.astype(int)
        X_te = test_df[feat_cols];          y_te = test_df["is_move"].values.astype(int)
        fr   = test_df["future_return"].values

        logger.info("Fold %d | train=%d val=%d test=%d MOVE_rate=%.1f%%",
                    s["fold"], len(X_tr), len(X_vl), len(X_te), 100*y_tr.mean())

        model   = fit_lgbm_binary(X_tr, y_tr, X_vl, y_vl, is_unbalance=True)
        y_pred  = model.predict(X_te)
        logger.info("  Best iter: %d", model.best_iteration_)

        m_met = binary_report(f"LightGBM fold {s['fold']}", y_te, y_pred, fr,
                              pos_label=1, class_names=["NO_MOVE","MOVE"], verbose=True)
        all_model.append(m_met)

        for bname, bpred in run_task_a_baselines(X_tr.values, X_te.values, feat_cols).items():
            bmet = binary_report(bname, y_te, bpred, fr,
                                 pos_label=1, class_names=["NO_MOVE","MOVE"], verbose=False)
            all_base.setdefault(bname, []).append(bmet)

        compare_model_vs_baselines(m_met, {k: v[-1] for k,v in all_base.items()})

    agg = aggregate_fold_metrics(all_model)
    best_bl_mcc = max(np.mean([m["mcc"] for m in v]) for v in all_base.values())
    model_mcc   = agg["mcc_mean"]

    print("\n" + "="*60)
    print("  FINAL VERDICT — Task A")
    print("="*60)
    print(f"  Model MCC mean : {model_mcc:+.4f}")
    print(f"  Best base MCC  : {best_bl_mcc:+.4f}")
    if model_mcc > best_bl_mcc + 0.02 and model_mcc > 0.10:
        print("  → USEFUL SIGNAL: model is a meaningful MOVE filter.")
    elif model_mcc > best_bl_mcc:
        print("  → MARGINAL EDGE: beats baselines but MCC < 0.10.")
    else:
        print("  → NO RELIABLE PREDICTIVE EDGE demonstrated at this horizon.")
    print("="*60)

    # Save final model
    os.makedirs(config.MODELS_DIR, exist_ok=True)
    last = splits[-1]
    full_tr = df.iloc[:last["test"][0]].reset_index(drop=True)
    vs  = int(len(full_tr) * config.WF_VAL_RATIO)
    Xf  = full_tr[feat_cols];  yf = full_tr["is_move"].values.astype(int)
    mdl = fit_lgbm_binary(Xf[:-vs], yf[:-vs], Xf[-vs:], yf[-vs:], is_unbalance=True)
    mp  = os.path.join(config.MODELS_DIR, "task_a_v2_lgbm.txt")
    mdl.booster_.save_model(mp)
    logger.info("Saved Task A model -> %s", mp)

    schema = {
        "task": "A", "feature_columns": feat_cols,
        "horizon": config.TASK_A_HORIZON, "k": config.TASK_A_K,
        "vol_win": config.TASK_A_VOL_WIN, "model_mcc_mean": model_mcc,
        "strategy": agg.get("strategy_name", "unknown"),
    }
    with open(os.path.join(config.MODELS_DIR, "task_a_v2_schema.json"), "w") as f:
        json.dump(schema, f, indent=2)

    return agg


# ────────────────────────────────────────────────────────────────────────────
# Task B: UP vs DOWN on MOVE bars (15m)
# ────────────────────────────────────────────────────────────────────────────

def run_task_b(df_1m: pd.DataFrame, feat_cols: list, n_folds: int):
    logger.info("\n%s\nTask B: UP vs DOWN — MOVE bars only (1m)\n%s", "═"*60, "═"*60)

    # Use df_1m directly — caller already ensured it is unmodified
    df = make_vol_scaled_targets(
        df_1m,
        horizon=config.TASK_B_HORIZON,
        k=config.TASK_B_K,
        vol_win=config.TASK_B_VOL_WIN,
        feature_cols=feat_cols,
    )

    logger.info("MOVE bars: %d / %d (%.1f%%)",
                int((df["is_move"]==1).sum()), len(df),
                100*(df["is_move"]==1).mean())

    splits   = expanding_window_splits(len(df), n_folds=n_folds, val_ratio=config.WF_VAL_RATIO)
    all_model, all_base = [], {}

    for s in splits:
        train_df, val_df, test_df = slice_fold(df, s)

        tr_m = train_df[train_df["is_move"]==1].reset_index(drop=True)
        vl_m = val_df[val_df["is_move"]==1].reset_index(drop=True)
        te_m = test_df[test_df["is_move"]==1].reset_index(drop=True)

        if len(tr_m) < 50 or len(te_m) < 10:
            logger.warning("Fold %d: too few MOVE bars — skipping.", s["fold"])
            continue

        X_tr = tr_m[feat_cols];         y_tr = tr_m["direction"].values.astype(int)
        X_vl = vl_m[feat_cols];         y_vl = vl_m["direction"].values.astype(int)
        X_te = te_m[feat_cols];         y_te = te_m["direction"].values.astype(int)
        fr   = te_m["future_return"].values

        logger.info("Fold %d | MOVE: train=%d val=%d test=%d UP_rate=%.1f%%",
                    s["fold"], len(X_tr), len(X_vl), len(X_te), 100*y_tr.mean())

        model  = fit_lgbm_binary(X_tr, y_tr, X_vl, y_vl, is_unbalance=False)
        y_pred = model.predict(X_te)
        logger.info("  Best iter: %d", model.best_iteration_)

        m_met = binary_report(f"LightGBM fold {s['fold']}", y_te, y_pred, fr,
                              pos_label=1, class_names=["DOWN","UP"], verbose=True)
        all_model.append(m_met)

        for bname, bpred in run_task_b_baselines(X_te.values, feat_cols).items():
            bmet = binary_report(bname, y_te, bpred, fr,
                                 pos_label=1, class_names=["DOWN","UP"], verbose=False)
            all_base.setdefault(bname, []).append(bmet)

        compare_model_vs_baselines(m_met, {k: v[-1] for k,v in all_base.items()})

    if not all_model:
        logger.error("No Task B folds completed.")
        return {}

    agg = aggregate_fold_metrics(all_model)
    best_bl_mcc = max(np.mean([m["mcc"] for m in v]) for v in all_base.values())
    model_mcc   = agg["mcc_mean"]

    print("\n" + "="*60)
    print("  FINAL VERDICT — Task B")
    print("="*60)
    print(f"  Model MCC mean : {model_mcc:+.4f}")
    print(f"  Best base MCC  : {best_bl_mcc:+.4f}")
    if model_mcc > best_bl_mcc + 0.02 and model_mcc > 0.10:
        print("  → USEFUL SIGNAL: direction prediction has real edge.")
    elif model_mcc > best_bl_mcc:
        print("  → MARGINAL EDGE: beats baselines but MCC < 0.10.")
    else:
        print("  → NO RELIABLE PREDICTIVE EDGE demonstrated.")
    print("="*60)

    # Save model
    last = splits[-1]
    full_df_move = df.iloc[:last["test"][0]]
    full_df_move = full_df_move[full_df_move["is_move"]==1].reset_index(drop=True)
    vs  = int(len(full_df_move) * config.WF_VAL_RATIO)
    Xf  = full_df_move[feat_cols];  yf = full_df_move["direction"].values.astype(int)
    mdl = fit_lgbm_binary(Xf[:-vs], yf[:-vs], Xf[-vs:], yf[-vs:], is_unbalance=False)
    mp  = os.path.join(config.MODELS_DIR, "task_b_v2_lgbm.txt")
    mdl.booster_.save_model(mp)
    logger.info("Saved Task B model -> %s", mp)

    schema = {
        "task": "B", "feature_columns": feat_cols,
        "resample": "15min", "horizon": config.TASK_B_HORIZON,
        "k": config.TASK_B_K, "vol_win": config.TASK_B_VOL_WIN,
        "model_mcc_mean": model_mcc,
    }
    with open(os.path.join(config.MODELS_DIR, "task_b_v2_schema.json"), "w") as f:
        json.dump(schema, f, indent=2)

    return agg


# ────────────────────────────────────────────────────────────────────────────
# Entry point
# ────────────────────────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser()
    p.add_argument("--task",     choices=["a","b","both"], default="both")
    p.add_argument("--n-folds",  type=int, default=config.WF_N_FOLDS)
    p.add_argument("--max-rows", type=int, default=None)
    p.add_argument("--ohlcv",    default=OHLCV_CSV)
    p.add_argument("--features", default=FEATURES_PARQUET)
    p.add_argument("--strategy", default=None,
                   help="Path to a strategy JSON (e.g. strategies/flowgate_1m_v1.json). "
                        "If provided, restricts features to those listed in feature_layers.")
    return p.parse_args()


def load_strategy(path: str):
    with open(path) as f:
        return json.load(f)


def apply_regime_filter(df: pd.DataFrame, strategy: dict) -> pd.DataFrame:
    """Filter rows based on regime_filter defined in strategy JSON."""
    rf = strategy.get("regime_filter")
    if not rf:
        return df
    col = rf["column"]
    if col not in df.columns:
        logger.warning("Regime filter column '%s' not in data — skipping filter", col)
        return df
    mode = rf["mode"]
    threshold = rf["threshold"]
    before = len(df)
    if mode == "gt":
        df = df[df[col] > threshold].reset_index(drop=True)
    elif mode == "lt":
        df = df[df[col] < threshold].reset_index(drop=True)
    elif mode == "abs_gt":
        df = df[df[col].abs() > threshold].reset_index(drop=True)
    logger.info("Regime filter '%s' %s %.4f: %d → %d rows (%.1f%% kept)",
                col, mode, threshold, before, len(df), 100*len(df)/before)
    return df


def _check_ram():
    """Abort early if available RAM is critically low (< 3 GB)."""
    import subprocess
    try:
        # macOS: get free + inactive pages via vm_stat
        out = subprocess.check_output(["vm_stat"], text=True)
        page_size = 16384  # bytes, standard on Apple Silicon / modern Intel Macs
        free = inactive = 0
        for line in out.splitlines():
            if line.startswith("Pages free:"):
                free = int(line.split(":")[1].strip().rstrip("."))
            elif line.startswith("Pages inactive:"):
                inactive = int(line.split(":")[1].strip().rstrip("."))
        available_gb = (free + inactive) * page_size / 1e9
        logger.info("Available RAM (free + inactive): %.1f GB", available_gb)
        if available_gb < 3.0:
            logger.error(
                "Only %.1f GB RAM available — risk of OOM crash. "
                "Close other apps or use --max-rows to limit data.",
                available_gb,
            )
            sys.exit(1)
    except Exception:
        pass  # non-macOS or vm_stat unavailable — skip check


def main():
    args = parse_args()

    _check_ram()
    feature_metadata = validate_feature_provenance(args.features)

    strategy = None
    strategy_name = "all-features"
    if args.strategy:
        strategy = load_strategy(args.strategy)
        strategy_name = f"{strategy['name']}-{strategy['version']}"

    logger.info("═"*60)
    logger.info("train_v2  —  strategy: %s", strategy_name)
    logger.info("Task: %s  |  Folds: %d  |  Max rows: %s",
                args.task, args.n_folds, args.max_rows or "all")
    logger.info("Feature source: %s  |  Stage: %s",
                feature_metadata.get("source"), feature_metadata.get("pipeline_stage"))
    logger.info("═"*60)

    df_1m = load_merged(args.ohlcv, args.features, args.max_rows)

    if strategy:
        wanted = [f for layer in strategy["feature_layers"].values() for f in layer]
        feat_cols = [c for c in wanted if c in df_1m.columns]
        missing = [c for c in wanted if c not in df_1m.columns]
        if missing:
            logger.warning("Strategy features not found in data: %s", missing)
        # Apply regime filter if defined
        df_1m = apply_regime_filter(df_1m, strategy)
    else:
        feat_cols = [c for c in pd.read_parquet(args.features).columns
                     if c != "timestamp_ms" and c in df_1m.columns]

    logger.info("Features selected: %d", len(feat_cols))

    if args.task in ("a", "both"):
        # make_vol_scaled_targets copies internally — no need to copy here
        run_task_a(df_1m, feat_cols, args.n_folds)
        gc.collect()

    if args.task in ("b", "both"):
        # df_1m is still clean (no target columns added) — pass directly
        run_task_b(df_1m, feat_cols, args.n_folds)
        gc.collect()


if __name__ == "__main__":
    main()
