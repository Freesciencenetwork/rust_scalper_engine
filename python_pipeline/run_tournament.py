"""
run_tournament.py — Train all strategies and compare results.

Trains Task B only (direction signal — where edge lives) for each strategy.
ConfidenceGate uses FlowGate's already-trained model with a threshold applied.
All others train fresh with train_v2.py.

Usage: python3 run_tournament.py
"""

import json
import os
import subprocess
import sys
import warnings
warnings.filterwarnings("ignore")

import numpy as np
import pandas as pd
from lightgbm import Booster
from sklearn.metrics import matthews_corrcoef, accuracy_score

BASE       = os.path.dirname(os.path.abspath(__file__))
CKPT_FG    = os.path.join(BASE, "models/checkpoints/flowgate_1m_v1_20260422")
OHLCV_CSV  = os.path.join(BASE, "../src/historical_data/btcusd_1-min_data.csv")
FEATS_PARQ = os.path.join(BASE, "data/features_normalized.parquet")
HORIZON    = 3
N_PER_YEAR = 2000
SEED       = 42

STRATEGIES = [
    "strategies/confidence_gate_v1.json",
    "strategies/mean_reversion_vwap_v1.json",
    "strategies/regime_switcher_trending_v1.json",
    "strategies/regime_switcher_ranging_v1.json",
    "strategies/sweep_hunter_v1.json",
    "strategies/vwap_sniper_v1.json",
    "strategies/trend_rider_v1.json",
    "strategies/vol_breakout_v1.json",
]

# ── Helpers ──────────────────────────────────────────────────────────────────

def stratified_sample(df, n_per_year, seed):
    parts = []
    for _, grp in df.groupby("year"):
        parts.append(grp.sample(n=min(n_per_year, len(grp)), random_state=seed))
    return pd.concat(parts).sort_values("timestamp_ms").reset_index(drop=True)


def eval_model(booster, X, y_true, future_returns, threshold=0.5):
    """Evaluate a booster with optional confidence threshold."""
    proba = booster.predict(X)
    if threshold > 0.5:
        confident = (proba >= threshold) | (proba <= (1 - threshold))
        X_c      = X[confident]
        y_c      = y_true[confident]
        fr_c     = future_returns[confident]
        proba_c  = proba[confident]
        coverage = confident.mean() * 100
    else:
        y_c, fr_c, proba_c = y_true, future_returns, proba
        coverage = 100.0

    pred = (proba_c >= 0.5).astype(int)
    valid = ~np.isnan(y_c.astype(float))
    y_c, pred, fr_c = y_c[valid].astype(int), pred[valid], fr_c[valid]

    if len(y_c) < 10:
        return {"mcc": np.nan, "accuracy": np.nan, "coverage_pct": coverage,
                "avg_ret_down": np.nan, "avg_ret_up": np.nan, "n": 0}

    mcc = matthews_corrcoef(y_c, pred)
    acc = accuracy_score(y_c, pred)
    avg_down = fr_c[pred == 0].mean() if (pred == 0).sum() > 0 else np.nan
    avg_up   = fr_c[pred == 1].mean() if (pred == 1).sum() > 0 else np.nan
    return {"mcc": mcc, "accuracy": acc, "coverage_pct": coverage,
            "avg_ret_down": avg_down, "avg_ret_up": avg_up, "n": len(y_c)}


# ── Load test data ────────────────────────────────────────────────────────────

print("Loading data...")
ohlcv = pd.read_csv(OHLCV_CSV)
ohlcv.columns = ohlcv.columns.str.lower()
for c in ("open","high","low","close","volume"):
    ohlcv[c] = pd.to_numeric(ohlcv[c], errors="coerce")
ohlcv.dropna(subset=["open","high","low","close","volume"], inplace=True)
ohlcv["timestamp_ms"] = (ohlcv["timestamp"].astype(float) * 1000).astype("int64")
ohlcv.sort_values("timestamp_ms", inplace=True)
ohlcv["future_return"]  = ohlcv["close"].shift(-HORIZON) / ohlcv["close"] - 1
ohlcv["direction_true"] = np.where(ohlcv["future_return"] > 0, 1,
                          np.where(ohlcv["future_return"] < 0, 0, np.nan))
ohlcv["datetime"] = pd.to_datetime(ohlcv["timestamp_ms"], unit="ms", utc=True)
ohlcv["year"]     = ohlcv["datetime"].dt.year
ohlcv.dropna(subset=["direction_true"], inplace=True)

feats = pd.read_parquet(FEATS_PARQ)
merged = pd.merge(ohlcv[["timestamp_ms","close","year","future_return","direction_true"]],
                  feats, on="timestamp_ms", how="inner")
merged["year"] = pd.to_datetime(merged["timestamp_ms"], unit="ms", utc=True).dt.year
sample = stratified_sample(merged, N_PER_YEAR, SEED)
print(f"  Test sample: {len(sample):,} rows across {sample['year'].nunique()} years")

y_true_all  = sample["direction_true"].values
fr_all      = sample["future_return"].values

results = {}

# ── ConfidenceGate: reuse FlowGate model, apply threshold ────────────────────

print("\n[1/8] ConfidenceGate — reusing FlowGate model with threshold=0.60")
try:
    fg_strategy = json.load(open(os.path.join(CKPT_FG, "strategy.json")))
    fg_feats    = [f for layer in fg_strategy["feature_layers"].values() for f in layer]
    fg_feats    = [c for c in fg_feats if c in sample.columns]
    booster_fg  = Booster(model_file=os.path.join(CKPT_FG, "task_b_lgbm.txt"))
    X_fg        = sample[fg_feats].values
    results["ConfidenceGate-1m (t=0.60)"] = eval_model(booster_fg, X_fg, y_true_all, fr_all, threshold=0.60)
    results["ConfidenceGate-1m (t=0.65)"] = eval_model(booster_fg, X_fg, y_true_all, fr_all, threshold=0.65)
    results["FlowGate-1m-v1 (baseline)"]  = eval_model(booster_fg, X_fg, y_true_all, fr_all, threshold=0.50)
    print("  Done.")
except Exception as e:
    print(f"  ERROR: {e}")

# ── Train + evaluate remaining strategies ────────────────────────────────────

for i, strategy_path in enumerate(STRATEGIES[1:], start=2):
    strat = json.load(open(os.path.join(BASE, strategy_path)))
    name  = f"{strat['name']}-{strat['version']}"
    print(f"\n[{i}/8] Training {name}...")

    result = subprocess.run(
        [sys.executable, "train_v2.py", "--task", "b",
         "--strategy", strategy_path, "--n-folds", "5"],
        capture_output=True, text=True, cwd=BASE
    )

    if result.returncode != 0:
        print(f"  FAILED: {result.stderr[-500:]}")
        results[name] = {"mcc": np.nan, "accuracy": np.nan, "coverage_pct": 0,
                         "avg_ret_down": np.nan, "avg_ret_up": np.nan, "n": 0}
        continue

    # Extract MCC from training output
    lines = result.stdout.splitlines()
    fold_mccs = [float(l.split("MCC=")[1].split()[0].replace("←","").strip())
                 for l in lines if "LightGBM fold" in l and "MCC=" in l and "←" in l]
    train_mcc = np.mean(fold_mccs) if fold_mccs else np.nan
    print(f"  Train MCC (walk-forward mean): {train_mcc:+.4f}")

    # Evaluate on test sample
    model_path = os.path.join(BASE, "models/task_b_v2_lgbm.txt")
    if not os.path.exists(model_path):
        print("  No model file found.")
        results[name] = {"mcc": np.nan, "accuracy": np.nan, "coverage_pct": 0,
                         "avg_ret_down": np.nan, "avg_ret_up": np.nan, "n": 0, "train_mcc": train_mcc}
        continue

    booster = Booster(model_file=model_path)
    wanted  = [f for layer in strat["feature_layers"].values() for f in layer]
    feats_s = [c for c in wanted if c in sample.columns]

    # Apply regime filter for test sample too
    rf = strat.get("regime_filter")
    if rf and rf["column"] in sample.columns:
        col, mode, thr = rf["column"], rf["mode"], rf["threshold"]
        if mode == "gt":    mask = sample[col] > thr
        elif mode == "lt":  mask = sample[col] < thr
        elif mode == "abs_gt": mask = sample[col].abs() > thr
        else: mask = pd.Series([True]*len(sample))
        s = sample[mask].reset_index(drop=True)
        yt = s["direction_true"].values
        fr = s["future_return"].values
        pct = mask.mean()*100
        print(f"  Regime filter keeps {pct:.1f}% of test bars")
    else:
        s, yt, fr = sample, y_true_all, fr_all

    X_s = s[feats_s].values
    r   = eval_model(booster, X_s, yt, fr)
    r["train_mcc"] = train_mcc
    results[name]  = r
    print(f"  Test MCC: {r['mcc']:+.4f}  Accuracy: {r['accuracy']:.4f}  Coverage: {r['coverage_pct']:.1f}%")

# ── Final scoreboard ─────────────────────────────────────────────────────────

print("\n" + "═"*80)
print(f"  {'STRATEGY':<38} {'Train MCC':>10} {'Test MCC':>10} {'Acc':>7} {'Cover':>7} {'DOWN ret':>10} {'UP ret':>8}")
print("═"*80)

sorted_results = sorted(
    [(k, v) for k, v in results.items() if isinstance(v.get("mcc"), float) and not np.isnan(v["mcc"])],
    key=lambda x: x[1]["mcc"], reverse=True
)
skipped = [(k, v) for k, v in results.items() if not isinstance(v.get("mcc"), float) or np.isnan(v["mcc"])]

for name, r in sorted_results:
    train_mcc = r.get("train_mcc", np.nan)
    tmcc_s = f"{train_mcc:+.4f}" if not np.isnan(train_mcc) else "  n/a  "
    print(f"  {name:<38} {tmcc_s:>10} {r['mcc']:>+10.4f} {r['accuracy']:>7.4f} {r['coverage_pct']:>6.1f}% "
          f"{r['avg_ret_down']:>+10.6f} {r['avg_ret_up']:>+8.6f}")

for name, r in skipped:
    print(f"  {name:<38}   FAILED or insufficient data")

if sorted_results:
    winner = sorted_results[0]
    print(f"\n  WINNER: {winner[0]}  (Test MCC={winner[1]['mcc']:+.4f})")
print("═"*80)

# Save results to memory.json
try:
    mem_path = os.path.join(BASE, "../memory.json")
    with open(mem_path) as f:
        mem = json.load(f)

    entry = {
        "index": mem["next_message_index"],
        "role": "assistant",
        "timestamp": pd.Timestamp.now().isoformat(),
        "model": "claude-sonnet-4-6",
        "content": "Ran full strategy tournament. Results recorded.",
        "metadata": {
            "tournament_sample": f"{len(sample)} rows, {sample['year'].nunique()} years",
            "results": {k: {kk: (float(vv) if isinstance(vv, (float, np.floating)) else vv)
                            for kk, vv in v.items()} for k, v in results.items()},
            "winner": sorted_results[0][0] if sorted_results else "none"
        }
    }
    mem["log"].append(entry)
    mem["next_message_index"] += 1
    with open(mem_path, "w") as f:
        json.dump(mem, f, indent=2)
    print("\n  memory.json updated.")
except Exception as e:
    print(f"\n  Could not update memory.json: {e}")
