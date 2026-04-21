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
