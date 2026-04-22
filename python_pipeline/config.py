"""
config.py — Central configuration for BTC direction prediction pipeline.

All tuneable knobs live here. Import this module everywhere instead of
hardcoding values so that a single change propagates through the whole
pipeline.
"""

import os

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
BASE_DIR   = os.path.dirname(os.path.abspath(__file__))
DATA_DIR   = os.path.join(BASE_DIR, "data")
MODELS_DIR = os.path.join(BASE_DIR, "models")

DATA_PATH          = os.path.join(DATA_DIR,   "btc_ohlcv.csv")
XGB_MODEL_PATH     = os.path.join(MODELS_DIR, "btc_xgb.json")
LGBM_MODEL_PATH    = os.path.join(MODELS_DIR, "btc_lgbm.txt")
SCHEMA_PATH        = os.path.join(MODELS_DIR, "feature_schema.json")

# ---------------------------------------------------------------------------
# Target creation
# ---------------------------------------------------------------------------
# horizon: how many bars ahead to measure the future return
# threshold: minimum absolute move to label as UP / DOWN (avoids training on noise)
HORIZON   = 5
THRESHOLD = 0.001      # 0.1 %

# ---------------------------------------------------------------------------
# Class labels
# ---------------------------------------------------------------------------
# 0 = DOWN, 1 = FLAT, 2 = UP  — kept as ints everywhere
CLASS_DOWN = 0
CLASS_FLAT = 1
CLASS_UP   = 2
NUM_CLASSES = 3

# ---------------------------------------------------------------------------
# Time-based split ratios   (must sum to 1.0)
# ---------------------------------------------------------------------------
TRAIN_RATIO = 0.70
VAL_RATIO   = 0.15
TEST_RATIO  = 0.15      # 1 - TRAIN_RATIO - VAL_RATIO

# ---------------------------------------------------------------------------
# Reproducibility
# ---------------------------------------------------------------------------
RANDOM_SEED = 42

# ---------------------------------------------------------------------------
# XGBoost hyper-parameters
# ---------------------------------------------------------------------------
XGB_PARAMS = {
    "objective"          : "multi:softprob",
    "num_class"          : NUM_CLASSES,
    "tree_method"        : "hist",          # fastest CPU trainer
    "n_estimators"       : 2000,
    "max_depth"          : 4,
    "learning_rate"      : 0.03,
    "subsample"          : 0.8,
    "colsample_bytree"   : 0.8,
    "seed"               : RANDOM_SEED,
    "eval_metric"        : "mlogloss",
    "early_stopping_rounds": 100,
    "verbosity"          : 0,               # silence XGBoost's own logging
}

# ---------------------------------------------------------------------------
# LightGBM hyper-parameters
# ---------------------------------------------------------------------------
LGBM_PARAMS = {
    "objective"          : "multiclass",
    "num_class"          : NUM_CLASSES,
    "n_estimators"       : 2000,
    "learning_rate"      : 0.03,
    "num_leaves"         : 31,
    "subsample"          : 0.8,
    "colsample_bytree"   : 0.8,
    "random_state"       : RANDOM_SEED,
    "n_jobs"             : -1,
    "verbose"            : -1,              # silence LightGBM's own logging
}
LGBM_EARLY_STOPPING_ROUNDS = 100

# ---------------------------------------------------------------------------
# Rolling-window sizes used in features.py
# ---------------------------------------------------------------------------
ROLLING_MEAN_WIN  = 20
ROLLING_STD_WIN   = 20
RSI_PERIOD        = 14
MACD_FAST         = 12
MACD_SLOW         = 26
MACD_SIGNAL       = 9
EMA_FAST          = 9
EMA_SLOW          = 21
VOL_ZSCORE_WIN    = 20

# ---------------------------------------------------------------------------
# Task A — MOVE vs NO_MOVE  (binary, 1-minute candles)
#
# MOVE definition:
#   rolling_vol[t] = std( ret_1[t-VOL_WIN : t] )    <- past data only
#   future_ret[t]  = (close[t+H] - close[t]) / close[t]
#   MOVE if abs(future_ret[t]) > K * rolling_vol[t]
#
# K is the signal-to-noise multiplier. K=1.0 means any move larger than
# one rolling-std is labelled MOVE. Raise K to make the task harder but
# more directionally clean; lower K to increase MOVE frequency.
# ---------------------------------------------------------------------------
TASK_A_HORIZON   = 5      # bars ahead (= 5 minutes on 1m data)
TASK_A_VOL_WIN   = 20     # Rust hist_vol_logrets_20 window (kept for reference only)
TASK_A_K         = 3.7    # threshold multiplier for trend_hist_vol_logrets_20
                           # k=3.7 gives ~44% MOVE rate, matching the original
                           # rolling_std(ret_1, 60) k=1.0 baseline
TASK_A_LABEL_MOVE   = 1
TASK_A_LABEL_NOMOVE = 0

# ---------------------------------------------------------------------------
# Task B — UP vs DOWN on MOVE bars only  (binary, 15-minute candles)
# Same vol-scaled formula but applied to resampled 15m candles.
# ---------------------------------------------------------------------------
TASK_B_HORIZON  = 3       # bars ahead (= 3 minutes on 1m data)
TASK_B_VOL_WIN  = 20      # Rust hist_vol_logrets_20 window (kept for reference only)
TASK_B_K        = 3.7     # same multiplier as Task A for consistency
TASK_B_LABEL_UP   = 1
TASK_B_LABEL_DOWN = 0

# ---------------------------------------------------------------------------
# Walk-forward cross-validation
# ---------------------------------------------------------------------------
WF_N_FOLDS    = 5     # number of test folds (expanding window)
WF_VAL_RATIO  = 0.15  # fraction of training window held out for early-stopping

# ---------------------------------------------------------------------------
# LightGBM — binary tasks  (Task A and Task B)
# is_unbalance=True compensates for MOVE/NO_MOVE skew without manual weighting.
# ---------------------------------------------------------------------------
LGBM_BINARY_PARAMS = {
    "objective"       : "binary",
    "n_estimators"    : 500,
    "learning_rate"   : 0.05,
    "num_leaves"      : 31,
    "max_depth"       : -1,
    "subsample"       : 0.8,
    "colsample_bytree": 0.8,
    "min_child_samples": 20,
    "random_state"    : RANDOM_SEED,
    "n_jobs"          : 4,          # cap threads — n_jobs=-1 copies data per core
    "verbose"         : -1,
    "is_unbalance"    : True,
}
LGBM_BINARY_EARLY_STOPPING = 50
