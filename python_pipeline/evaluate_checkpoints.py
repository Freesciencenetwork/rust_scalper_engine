"""
evaluate_checkpoints.py — Benchmark all checkpoints against N random windows
drawn from the full BTC 1m historical dataset.

All features come exclusively from the Rust-computed parquet
(data/features_normalized.parquet).  No indicator or formula is
recomputed in Python.

The only Python-side computation is the training label:
  future_return = close[t+H] / close[t] - 1
This is inherently forward-looking and cannot come from the Rust engine.
The MOVE threshold uses Rust's atr_pct (previous bar, shift(1)) instead
of a hand-rolled rolling-std.

The old 3-class model (btc_1m_3class_*) is excluded: it was trained on
Python-computed OHLCV features (RSI, MACD, ret_N) that are not in the
Rust parquet, so it cannot be evaluated on the same feature set.

Usage
-----
    python3 evaluate_checkpoints.py              # 10 windows, 10 000 bars each
    python3 evaluate_checkpoints.py --n 20
    python3 evaluate_checkpoints.py --window 5000
"""

import argparse
import json
import sys
import warnings
from datetime import datetime
from pathlib import Path

warnings.filterwarnings("ignore")

import numpy as np
import pandas as pd
from lightgbm import Booster
from sklearn.metrics import matthews_corrcoef, accuracy_score

# ── Paths ─────────────────────────────────────────────────────────────────────
BASE       = Path(__file__).parent
OHLCV_CSV  = BASE / "../src/historical_data/btcusd_1-min_data.csv"
FEATS_PARQ = BASE / "data/features_normalized.parquet"
CKPT_DIR   = BASE / "models/checkpoints"

# ── Label config (must match training) ───────────────────────────────────────
TASK_A_HORIZON = 5
TASK_A_K       = 1.0

TASK_B_HORIZON = 3
TASK_B_K       = 1.0

SEED = 42


# ── Data loading ──────────────────────────────────────────────────────────────

def load_merged() -> pd.DataFrame:
    """
    Merge OHLCV CSV with the Rust-computed normalized feature parquet.
    Returns a DataFrame containing close price (for label computation only)
    and all 129 Rust features.  No Python indicators are added.
    """
    print("Loading OHLCV …")
    raw = pd.read_csv(OHLCV_CSV)
    raw.columns = raw.columns.str.lower()
    for c in ("open", "high", "low", "close", "volume"):
        raw[c] = pd.to_numeric(raw[c], errors="coerce")
    raw.dropna(subset=["open", "high", "low", "close", "volume"], inplace=True)
    raw["timestamp_ms"] = (raw["timestamp"].astype(float) * 1000).astype("int64")
    raw.sort_values("timestamp_ms", inplace=True)
    raw.reset_index(drop=True, inplace=True)

    print("Loading Rust features …")
    feats = pd.read_parquet(FEATS_PARQ)

    print("Merging …")
    df = pd.merge(
        raw[["timestamp_ms", "close"]],   # only close needed — no Python features
        feats,
        on="timestamp_ms",
        how="inner",
    )
    df.sort_values("timestamp_ms", inplace=True)
    df.reset_index(drop=True, inplace=True)
    df["datetime"] = pd.to_datetime(df["timestamp_ms"], unit="ms", utc=True)
    df["year"]     = df["datetime"].dt.year

    print(f"  Merged: {len(df):,} rows  ({df['year'].min()} – {df['year'].max()})")
    return df


# ── Label computation (targets only — not features) ───────────────────────────

def add_labels(df: pd.DataFrame) -> pd.DataFrame:
    """
    Attach training labels to the merged DataFrame.

    future_return is the only formula computed in Python — it is a target
    (forward-looking by design) that the Rust engine cannot produce.

    The MOVE threshold uses atr_pct.shift(1) from the Rust parquet instead
    of a Python-computed rolling std.  shift(1) ensures no bar sees its own
    ATR in the threshold calculation.
    """
    # ── Task A labels (MOVE vs NO_MOVE) ──────────────────────────────────────
    fut_a          = df["close"].shift(-TASK_A_HORIZON) / df["close"] - 1
    vol_threshold_a = TASK_A_K * df["atr_pct"].shift(1)   # Rust ATR, no lookahead
    df["future_ret_a"] = fut_a
    df["is_move"]      = (fut_a.abs() > vol_threshold_a).astype("Int64")

    # ── Task B labels (UP vs DOWN on MOVE bars) ───────────────────────────────
    fut_b          = df["close"].shift(-TASK_B_HORIZON) / df["close"] - 1
    vol_threshold_b = TASK_B_K * df["atr_pct"].shift(1)
    df["future_ret_b"] = fut_b
    is_move_b      = (fut_b.abs() > vol_threshold_b)
    df["direction"] = np.where(
        is_move_b & (fut_b > 0), 1,
        np.where(is_move_b & (fut_b < 0), 0, np.nan),
    )
    df["is_move_b"] = is_move_b.astype("Int64")

    return df


# ── Window sampling ───────────────────────────────────────────────────────────

def sample_windows(df: pd.DataFrame, n: int, window: int, seed: int):
    rng    = np.random.default_rng(seed)
    years  = sorted(df["year"].unique())
    per_yr = max(1, n // len(years))
    wins   = []
    for yr in years:
        idx   = df.index[df["year"] == yr].tolist()
        valid = [i for i in idx if i + window <= len(df)]
        if not valid:
            continue
        chosen = rng.choice(valid, size=min(per_yr, len(valid)), replace=False)
        for s in chosen:
            wins.append((s, s + window))
    while len(wins) < n:
        s = rng.integers(0, len(df) - window)
        wins.append((int(s), int(s) + window))
    rng.shuffle(wins)
    return wins[:n]


# ── Checkpoint discovery ──────────────────────────────────────────────────────

def load_checkpoints():
    """
    Discover all Rust-feature checkpoints.
    Skips the old 3-class model — it was trained on Python-computed OHLCV
    indicators (ret_N, RSI, MACD) not present in the Rust parquet.
    """
    ckpts = []
    for d in sorted(CKPT_DIR.iterdir()):
        if not d.is_dir() or d.name.startswith(".") or d.name == "__pycache__":
            continue

        # Task B v2
        sb = d / "task_b_v2_schema.json"
        if sb.exists() and (d / "task_b_v2_lgbm.txt").exists():
            s = json.loads(sb.read_text())
            ckpts.append({
                "name"     : d.name,
                "task"     : "B",
                "model"    : Booster(model_file=str(d / "task_b_v2_lgbm.txt")),
                "feat_cols": s["feature_columns"],
                "label"    : "direction UP/DOWN on MOVE bars",
            })

        # Task A v2
        sa2 = d / "task_a_v2_schema.json"
        if sa2.exists() and (d / "task_a_v2_lgbm.txt").exists():
            s = json.loads(sa2.read_text())
            ckpts.append({
                "name"     : d.name,
                "task"     : "A",
                "model"    : Booster(model_file=str(d / "task_a_v2_lgbm.txt")),
                "feat_cols": s["feature_columns"],
                "label"    : "MOVE detect (129 Rust feats)",
            })

        # FlowGate Task A (30 curated Rust feats)
        sa = d / "task_a_schema.json"
        if sa.exists() and not sa2.exists() and (d / "task_a_lgbm.txt").exists():
            s = json.loads(sa.read_text())
            ckpts.append({
                "name"     : d.name,
                "task"     : "A",
                "model"    : Booster(model_file=str(d / "task_a_lgbm.txt")),
                "feat_cols": s["feature_columns"],
                "label"    : "MOVE detect (FlowGate 30 Rust feats)",
            })

        # Old 3-class: SKIP — Python-computed features, incompatible with Rust parquet
        if (d / "run_metadata.json").exists():
            print(f"  [skip] {d.name} — trained on Python-computed indicators, "
                  f"not evaluable with Rust features")

    return ckpts


# ── Evaluation ────────────────────────────────────────────────────────────────

def evaluate_window(ckpt: dict, window_df: pd.DataFrame):
    task      = ckpt["task"]
    feat_cols = ckpt["feat_cols"]
    model     = ckpt["model"]

    missing = [c for c in feat_cols if c not in window_df.columns]
    if missing:
        return None

    if task == "A":
        sub = window_df.dropna(subset=feat_cols + ["is_move"])
        if len(sub) < 20:
            return None
        X      = sub[feat_cols].values
        y_true = sub["is_move"].values.astype(int)
        proba  = model.predict(X)
        y_pred = (proba > 0.5).astype(int)
        return {
            "mcc": matthews_corrcoef(y_true, y_pred),
            "acc": accuracy_score(y_true, y_pred),
            "n"  : len(sub),
        }

    if task == "B":
        sub = window_df[window_df["is_move_b"] == 1].dropna(
            subset=feat_cols + ["direction"]
        )
        if len(sub) < 10:
            return None
        X      = sub[feat_cols].values
        y_true = sub["direction"].values.astype(int)
        proba  = model.predict(X)
        y_pred = (proba > 0.5).astype(int)
        return {
            "mcc": matthews_corrcoef(y_true, y_pred),
            "acc": accuracy_score(y_true, y_pred),
            "n"  : len(sub),
        }

    return None


# ── Formatting ────────────────────────────────────────────────────────────────

def print_and_collect(ckpts, windows, df):
    win_w = 22

    def row_line(label, vals):
        return "| " + f"{label:<{win_w}}" + " | " + " | ".join(
            f"{v:<14}" for v in vals
        ) + " |"

    short = {c["name"]: c["name"][-40:] for c in ckpts}
    sep   = "| " + "-"*win_w + " | " + " | ".join("-"*14 for _ in ckpts) + " |"
    hdr   = row_line("Window (UTC)", [short[c["name"]] for c in ckpts])

    lines_out = [hdr, sep]
    lines_md  = ["## Per-window results (MCC / Accuracy)\n", hdr, sep]

    per_mcc = {c["name"]: [] for c in ckpts}
    per_acc = {c["name"]: [] for c in ckpts}

    for s, e in windows:
        win_df = df.iloc[s:e]
        t0 = win_df["datetime"].iloc[0].strftime("%Y-%m-%d")
        t1 = win_df["datetime"].iloc[-1].strftime("%Y-%m-%d")
        label = f"{t0} → {t1}"

        vals = []
        for c in ckpts:
            res = evaluate_window(c, win_df)
            if res:
                per_mcc[c["name"]].append(res["mcc"])
                per_acc[c["name"]].append(res["acc"])
                vals.append(f"{res['mcc']:+.3f} / {res['acc']:.1%}")
            else:
                vals.append("skip")

        lines_out.append(row_line(label, vals))
        lines_md .append(row_line(label, vals))

    lines_out.append(sep)
    lines_md .append(sep)

    summary = []
    for c in ckpts:
        mccs = per_mcc[c["name"]]
        accs = per_acc[c["name"]]
        if mccs:
            summary.append((c["name"], np.mean(mccs), np.std(mccs), np.mean(accs)))
            cell = f"{np.mean(mccs):+.3f}±{np.std(mccs):.3f}"
        else:
            summary.append((c["name"], None, None, None))
            cell = "n/a"
        lines_out.append("")  # placeholder
        lines_md .append("")

    # overwrite the last N placeholders with the aggregate row
    agg_vals = [
        f"{m:+.3f}±{s:.3f}" if m is not None else "n/a"
        for _, m, s, _ in summary
    ]
    lines_out = lines_out[:-len(ckpts)] + [row_line("MEAN ± STD", agg_vals)]
    lines_md  = lines_md [:-len(ckpts)] + [row_line("MEAN ± STD", agg_vals)]

    # ranking block
    ranked = sorted(
        [(n, m, s, a) for n, m, s, a in summary if m is not None],
        key=lambda x: x[1], reverse=True,
    )
    rank_block = [
        "\n## Aggregate ranking (by mean MCC)\n",
        "| # | Checkpoint | Task | Mean MCC | ±Std | Mean Acc | Edge |",
        "| - | ---------- | ---- | -------- | ---- | -------- | ---- |",
    ]
    task_map = {c["name"]: c["task"] for c in ckpts}
    for i, (name, m, s, a) in enumerate(ranked, 1):
        edge = "YES" if m > 0.10 else ("MARGINAL" if m > 0 else "NO")
        rank_block.append(
            f"| {i} | {name} | {task_map[name]} | {m:+.4f} | ±{s:.4f} | {a:.1%} | {edge} |"
        )

    out_str = "\n".join(lines_out)
    md_str  = "\n".join(lines_md) + "\n" + "\n".join(rank_block)
    return out_str, md_str


# ── Entry point ───────────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser()
    p.add_argument("--n",      type=int, default=10,    help="Number of windows (default 10)")
    p.add_argument("--window", type=int, default=10000, help="Bars per window (default 10 000)")
    p.add_argument("--seed",   type=int, default=SEED)
    return p.parse_args()


def main():
    args = parse_args()

    df = load_merged()
    df = add_labels(df)

    print("\nLoading checkpoints …")
    ckpts = load_checkpoints()
    if not ckpts:
        print("No compatible checkpoints found.")
        sys.exit(1)
    for c in ckpts:
        print(f"  ✓  {c['name']}  [{c['label']}]")

    print(f"\nSampling {args.n} windows of {args.window:,} bars …")
    windows = sample_windows(df, args.n, args.window, args.seed)
    windows.sort(key=lambda x: x[0])

    print("\nEvaluating …\n")
    out_str, md_str = print_and_collect(ckpts, windows, df)
    print(out_str)

    ts       = datetime.now().strftime("%Y-%m-%d %H:%M")
    md_full  = (
        f"# Checkpoint Evaluation — {args.n} Random Windows\n\n"
        f"Generated {ts}.  "
        f"Each window = {args.window:,} consecutive 1m bars (~{args.window // 1440}d).  "
        f"Windows drawn stratified by year.  "
        f"All features are Rust-computed (no Python indicators).\n\n"
        + md_str
        + "\n\n---\n*Run `python3 evaluate_checkpoints.py` to refresh.*\n"
    )
    out_path = CKPT_DIR / "EVAL_RESULTS.md"
    out_path.write_text(md_full)
    print(f"\nWrote {out_path}")


if __name__ == "__main__":
    main()
