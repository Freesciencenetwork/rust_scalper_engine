"""
compare_checkpoints.py — Head-to-head test of all checkpoint models
on a stratified random sample across the full 10-year BTC dataset.

Models compared:
  A) 1m_7.5M_lgbm_winner_20260421  — 3-class (DOWN/FLAT/UP), 15 OHLCV features
  B) task_a_v2_20260422             — binary MOVE/NO_MOVE, 129 Rust features
  C) flowgate_1m_v1_20260422        — Task A + Task B, 30 curated Rust features

Fair comparison metric: directional MCC and average future return per predicted class.
For model A: UP vs DOWN only (FLAT predictions excluded).
For model B: not directional — shown as MOVE recall for context only.
For model C: Task B direction predictions on MOVE bars.

Sample: N_PER_YEAR rows drawn randomly from each calendar year.
"""

import json
import os
import sys
import warnings
warnings.filterwarnings("ignore")

import numpy as np
import pandas as pd
from lightgbm import Booster
from sklearn.metrics import matthews_corrcoef, accuracy_score

# ── Paths ────────────────────────────────────────────────────────────────────
BASE        = os.path.dirname(os.path.abspath(__file__))
OHLCV_CSV   = os.path.join(BASE, "../src/historical_data/btcusd_1-min_data.csv")
FEATS_PARQ  = os.path.join(BASE, "data/features_normalized.parquet")
CKPT_DIR    = os.path.join(BASE, "models/checkpoints")

CKPT_OLD    = os.path.join(CKPT_DIR, "1m_7.5M_lgbm_winner_20260421")
CKPT_V2     = os.path.join(CKPT_DIR, "task_a_v2_20260422")
CKPT_FG     = os.path.join(CKPT_DIR, "flowgate_1m_v1_20260422")

N_PER_YEAR  = 2000   # rows sampled per calendar year
HORIZON     = 5      # bars ahead for future return
SEED        = 42

# ── Helpers ──────────────────────────────────────────────────────────────────

def load_ohlcv():
    print("Loading OHLCV...")
    df = pd.read_csv(OHLCV_CSV)
    df.columns = df.columns.str.lower()
    for c in ("open","high","low","close","volume"):
        df[c] = pd.to_numeric(df[c], errors="coerce")
    df.dropna(subset=["open","high","low","close","volume"], inplace=True)
    df["timestamp_ms"] = (df["timestamp"].astype(float) * 1000).astype("int64")
    df.sort_values("timestamp_ms", inplace=True)
    df.reset_index(drop=True, inplace=True)
    df["datetime"] = pd.to_datetime(df["timestamp_ms"], unit="ms", utc=True)
    df["year"] = df["datetime"].dt.year
    df["future_return"] = df["close"].shift(-HORIZON) / df["close"] - 1
    df["direction_true"] = np.where(df["future_return"] > 0, 1,
                           np.where(df["future_return"] < 0, 0, np.nan))
    df.dropna(subset=["future_return","direction_true"], inplace=True)
    print(f"  {len(df):,} rows, years {df['year'].min()}–{df['year'].max()}")
    return df


def stratified_sample(df, n_per_year, seed):
    parts = []
    for yr, grp in df.groupby("year"):
        n = min(n_per_year, len(grp))
        parts.append(grp.sample(n=n, random_state=seed))
    sample = pd.concat(parts).sort_values("timestamp_ms").reset_index(drop=True)
    print(f"  Sample: {len(sample):,} rows across {df['year'].nunique()} years")
    return sample


def build_old_features(df):
    """Compute the 15 OHLCV-derived features used by the 2026-04-21 model."""
    d = df.copy()
    d["ret_1"]        = d["close"].pct_change(1)
    d["ret_3"]        = d["close"].pct_change(3)
    d["ret_5"]        = d["close"].pct_change(5)
    d["roll_mean"]    = d["close"].rolling(20).mean()
    d["roll_std"]     = d["close"].rolling(20).std()
    d["close_to_mean"]= d["close"] / d["roll_mean"] - 1
    # RSI
    delta = d["close"].diff()
    gain  = delta.clip(lower=0).rolling(14).mean()
    loss  = (-delta.clip(upper=0)).rolling(14).mean()
    rs    = gain / loss.replace(0, np.nan)
    d["rsi"] = 100 - 100 / (1 + rs)
    # MACD
    ema12 = d["close"].ewm(span=12, adjust=False).mean()
    ema26 = d["close"].ewm(span=26, adjust=False).mean()
    d["macd_line"]   = ema12 - ema26
    d["macd_signal"] = d["macd_line"].ewm(span=9, adjust=False).mean()
    d["macd_hist"]   = d["macd_line"] - d["macd_signal"]
    # Volume
    d["vol_change"]  = d["volume"].pct_change(1)
    d["vol_zscore"]  = (d["volume"] - d["volume"].rolling(20).mean()) / d["volume"].rolling(20).std()
    # EMA
    d["ema_fast"]    = d["close"].ewm(span=9,  adjust=False).mean()
    d["ema_slow"]    = d["close"].ewm(span=21, adjust=False).mean()
    d["ema_spread"]  = d["ema_fast"] / d["ema_slow"] - 1
    cols = ["ret_1","ret_3","ret_5","roll_mean","roll_std","close_to_mean",
            "rsi","macd_line","macd_signal","macd_hist",
            "vol_change","vol_zscore","ema_fast","ema_slow","ema_spread"]
    return d[cols]


def print_results(name, y_true, y_pred, future_returns, pred_labels=("DOWN","UP")):
    mask = ~np.isnan(y_true.astype(float))
    y_true = y_true[mask].astype(int)
    y_pred = y_pred[mask].astype(int)
    fr     = future_returns[mask]

    mcc  = matthews_corrcoef(y_true, y_pred)
    acc  = accuracy_score(y_true, y_pred)
    n    = len(y_true)

    print(f"\n{'─'*52}")
    print(f"  {name}")
    print(f"{'─'*52}")
    print(f"  Samples         : {n:,}")
    print(f"  Accuracy        : {acc:.4f}")
    print(f"  MCC             : {mcc:+.4f}")
    for lbl, val in enumerate(pred_labels):
        mask_p = y_pred == lbl
        if mask_p.sum() > 0:
            avg_ret = fr[mask_p].mean()
            print(f"  Pred {val:>6} ({mask_p.sum():>6} bars)  avg return: {avg_ret:+.6f}")
    return mcc


# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    rng = np.random.default_rng(SEED)

    # ── Load base data ──
    ohlcv = load_ohlcv()

    # ── Stratified sample (indices into ohlcv) ──
    print("\nSampling...")
    sample = stratified_sample(ohlcv, N_PER_YEAR, SEED)
    sample_idx = sample.index.tolist()

    # ── Load Rust features for the sample rows ──
    print("\nLoading Rust features...")
    feats_full = pd.read_parquet(FEATS_PARQ)
    merged = pd.merge(
        ohlcv[["timestamp_ms","close","year","future_return","direction_true"]],
        feats_full,
        on="timestamp_ms", how="inner"
    )
    # Re-sample after merge (some rows may drop out)
    merged["year"] = pd.to_datetime(merged["timestamp_ms"], unit="ms", utc=True).dt.year
    merged_sample = stratified_sample(merged, N_PER_YEAR, SEED)
    print(f"  Merged sample: {len(merged_sample):,} rows")

    results = {}

    # ════════════════════════════════════════════════════════
    # MODEL A — 1m_7.5M_lgbm_winner (3-class, OHLCV features)
    # ════════════════════════════════════════════════════════
    print("\n" + "═"*52)
    print("  MODEL A: 1m_7.5M_lgbm_winner (3-class, 15 feats)")
    print("═"*52)
    try:
        mdl_a = Booster(model_file=os.path.join(CKPT_OLD, "btc_lgbm.txt"))
        # Build features on the full ohlcv first (rolling needs context)
        old_feats_full = build_old_features(ohlcv)
        old_feats_full["timestamp_ms"] = ohlcv["timestamp_ms"]
        old_feats_full["future_return"] = ohlcv["future_return"]
        old_feats_full["direction_true"] = ohlcv["direction_true"]
        old_feats_full["year"] = ohlcv["year"]
        old_feats_full.dropna(inplace=True)
        old_sample = stratified_sample(old_feats_full, N_PER_YEAR, SEED)

        feat_cols_old = ["ret_1","ret_3","ret_5","roll_mean","roll_std","close_to_mean",
                         "rsi","macd_line","macd_signal","macd_hist",
                         "vol_change","vol_zscore","ema_fast","ema_slow","ema_spread"]
        X_old = old_sample[feat_cols_old].values
        proba = mdl_a.predict(X_old)           # shape (n, 3)
        pred_class = np.argmax(proba, axis=1)  # 0=DOWN, 1=FLAT, 2=UP

        # Directional only: keep bars where model predicts DOWN or UP (not FLAT)
        dir_mask = pred_class != 1
        y_true_a  = old_sample["direction_true"].values[dir_mask]
        y_pred_a  = np.where(pred_class[dir_mask] == 2, 1, 0)  # UP=1, DOWN=0
        fr_a      = old_sample["future_return"].values[dir_mask]

        pct_flat = (pred_class == 1).mean() * 100
        print(f"  Predicts FLAT: {pct_flat:.1f}% of bars (excluded from directional score)")
        mcc_a = print_results("MODEL A — directional (non-FLAT only)", y_true_a, y_pred_a, fr_a)
        results["Model A (3-class OHLCV)"] = mcc_a
    except Exception as e:
        print(f"  ERROR: {e}")
        results["Model A (3-class OHLCV)"] = None

    # ════════════════════════════════════════════════════════
    # MODEL C — FlowGate-1m-v1 Task B (UP/DOWN, 30 feats)
    # ════════════════════════════════════════════════════════
    print("\n" + "═"*52)
    print("  MODEL C: FlowGate-1m-v1 Task B (UP/DOWN, 30 feats)")
    print("═"*52)
    try:
        mdl_c   = Booster(model_file=os.path.join(CKPT_FG, "task_b_lgbm.txt"))
        strategy = json.load(open(os.path.join(CKPT_FG, "strategy.json")))
        feat_cols_c = [f for layer in strategy["feature_layers"].values() for f in layer]
        avail_c = [c for c in feat_cols_c if c in merged_sample.columns]
        X_c = merged_sample[avail_c].values
        pred_c = (mdl_c.predict(X_c) > 0.5).astype(int)  # 1=UP, 0=DOWN
        y_true_c = merged_sample["direction_true"].values
        fr_c     = merged_sample["future_return"].values
        mcc_c = print_results("MODEL C — FlowGate Task B (all bars)", y_true_c, pred_c, fr_c)
        results["Model C (FlowGate Task B 30f)"] = mcc_c
    except Exception as e:
        print(f"  ERROR: {e}")
        results["Model C (FlowGate Task B 30f)"] = None

    # ════════════════════════════════════════════════════════
    # WINNER
    # ════════════════════════════════════════════════════════
    print("\n" + "═"*52)
    print("  FINAL SCOREBOARD")
    print("═"*52)
    for name, mcc in results.items():
        if isinstance(mcc, float):
            print(f"  {name:<38} MCC={mcc:+.4f}")
        else:
            print(f"  {name:<38} {mcc}")

    numeric = {k: v for k, v in results.items() if isinstance(v, float)}
    if numeric:
        winner = max(numeric, key=numeric.get)
        print(f"\n  WINNER: {winner}  (MCC={numeric[winner]:+.4f})")
    print("═"*52)


if __name__ == "__main__":
    main()
