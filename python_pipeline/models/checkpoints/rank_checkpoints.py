"""
rank_checkpoints.py — Auto-discovers all checkpoint folders, reads their
schema files, and writes RANKING.md with a sorted results table.

Run from any directory:
    python3 rank_checkpoints.py

Updates RANKING.md in the same folder as this script.
"""

import json
import os
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).parent


# ── Schema readers ────────────────────────────────────────────────────────────

def _read_task_schema(path: Path) -> list[dict]:
    """Parse a task_a/task_b schema JSON into one row per model."""
    with open(path) as f:
        s = json.load(f)
    task = s.get("task", "?")
    mcc  = s.get("model_mcc_mean")
    if mcc is None:
        return []
    label = "MOVE detect" if task == "A" else "Direction (UP/DOWN on MOVE bars)"
    resample = s.get("resample", "1m")
    horizon  = s.get("horizon", "?")
    k        = s.get("k", "?")
    n_feats  = len(s.get("feature_columns", []))
    edge     = "YES" if mcc > 0.10 else ("MARGINAL" if mcc > 0 else "NO")
    return [{
        "task"      : f"Task {task} — {label}",
        "timeframe" : resample if resample != "1m" else "1m",
        "horizon"   : f"{horizon}b",
        "k"         : k,
        "n_feats"   : n_feats,
        "mcc"       : mcc,
        "accuracy"  : None,
        "edge"      : edge,
    }]


def _read_run_metadata(path: Path) -> list[dict]:
    """Parse run_metadata.json (old 3-class format)."""
    with open(path) as f:
        m = json.load(f)
    lgbm = m.get("lightgbm", {})
    acc  = lgbm.get("accuracy")
    return [{
        "task"      : "3-class (DOWN / FLAT / UP)",
        "timeframe" : m.get("candle_interval", "1m"),
        "horizon"   : f"{m.get('horizon','?')}b",
        "k"         : "fixed thr",
        "n_feats"   : 15,
        "mcc"       : None,
        "accuracy"  : acc,
        "edge"      : "NO (accuracy misleading: FLAT dominates)",
    }]


def load_checkpoint(ckpt_dir: Path) -> list[dict]:
    """Return a list of result rows for a checkpoint folder."""
    rows = []

    # Priority order: specific schemas first, fallback to metadata
    for name in ("task_b_v2_schema.json", "task_a_v2_schema.json",
                 "task_b_schema.json",    "task_a_schema.json"):
        p = ckpt_dir / name
        if p.exists():
            rows.extend(_read_task_schema(p))

    if not rows:
        meta = ckpt_dir / "run_metadata.json"
        if meta.exists():
            rows.extend(_read_run_metadata(meta))

    for row in rows:
        row["checkpoint"] = ckpt_dir.name

    return rows


# ── Table formatting ──────────────────────────────────────────────────────────

def _mcc_str(mcc) -> str:
    if mcc is None:
        return "n/a"
    return f"{mcc:+.4f}"


def _acc_str(acc) -> str:
    if acc is None:
        return ""
    return f"{acc:.1%}"


def _rank_key(row) -> float:
    """Sort by MCC descending; rows without MCC go to the bottom."""
    return row["mcc"] if row["mcc"] is not None else -999


def build_table(rows: list[dict]) -> str:
    rows = sorted(rows, key=_rank_key, reverse=True)

    col_widths = {
        "rank"       : 4,
        "checkpoint" : max(len(r["checkpoint"]) for r in rows),
        "task"       : max(len(r["task"])       for r in rows),
        "tf"         : 9,
        "hz"         : 7,
        "feats"      : 5,
        "mcc"        : 7,
        "acc"        : 7,
        "edge"       : max(len(r["edge"])       for r in rows),
    }

    def sep() -> str:
        return (
            f"| {'-'*col_widths['rank']} "
            f"| {'-'*col_widths['checkpoint']} "
            f"| {'-'*col_widths['task']} "
            f"| {'-'*col_widths['tf']} "
            f"| {'-'*col_widths['hz']} "
            f"| {'-'*col_widths['feats']} "
            f"| {'-'*col_widths['mcc']} "
            f"| {'-'*col_widths['acc']} "
            f"| {'-'*col_widths['edge']} |"
        )

    def hdr() -> str:
        return (
            f"| {'#':<{col_widths['rank']}} "
            f"| {'Checkpoint':<{col_widths['checkpoint']}} "
            f"| {'Task':<{col_widths['task']}} "
            f"| {'Timeframe':<{col_widths['tf']}} "
            f"| {'Horizon':<{col_widths['hz']}} "
            f"| {'Feats':<{col_widths['feats']}} "
            f"| {'MCC':<{col_widths['mcc']}} "
            f"| {'Acc':<{col_widths['acc']}} "
            f"| {'Edge':<{col_widths['edge']}} |"
        )

    lines = [hdr(), sep()]
    for i, r in enumerate(rows, 1):
        lines.append(
            f"| {i:<{col_widths['rank']}} "
            f"| {r['checkpoint']:<{col_widths['checkpoint']}} "
            f"| {r['task']:<{col_widths['task']}} "
            f"| {r['timeframe']:<{col_widths['tf']}} "
            f"| {r['horizon']:<{col_widths['hz']}} "
            f"| {r['n_feats']:<{col_widths['feats']}} "
            f"| {_mcc_str(r['mcc']):<{col_widths['mcc']}} "
            f"| {_acc_str(r['accuracy']):<{col_widths['acc']}} "
            f"| {r['edge']:<{col_widths['edge']}} |"
        )

    return "\n".join(lines)


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    all_rows = []
    for ckpt_dir in sorted(HERE.iterdir()):
        if not ckpt_dir.is_dir() or ckpt_dir.name.startswith("."):
            continue
        rows = load_checkpoint(ckpt_dir)
        if rows:
            all_rows.extend(rows)
        else:
            print(f"  [skip] {ckpt_dir.name} — no readable schema found")

    if not all_rows:
        print("No checkpoints found.")
        return

    table = build_table(all_rows)

    # ── Print to stdout ───────────────────────────────────────────────────────
    print(table)

    # ── Write RANKING.md ──────────────────────────────────────────────────────
    out = HERE / "RANKING.md"
    generated = datetime.now().strftime("%Y-%m-%d %H:%M")
    content = f"# Checkpoint Ranking\n\nSorted by MCC (descending). Generated {generated}.\n\n{table}\n\n---\n*Run `python3 rank_checkpoints.py` to refresh.*\n"
    out.write_text(content)
    print(f"\nWrote {out}")


if __name__ == "__main__":
    main()
