# `python_pipeline` file tree

Generated: 2026-04-22. Paths are relative to repo root unless noted.

```
python_pipeline/
├── data/
│   ├── btc_ohlcv.csv
│   ├── features_normalized.parquet
│   └── indicators_full.parquet
├── models/
│   ├── checkpoints/
│   │   ├── btc_1m_3class_down-flat-up_lgbm_acc68pct_noedge_20260421/
│   │   │   ├── btc_lgbm.txt
│   │   │   ├── btc_xgb.json
│   │   │   ├── feature_schema.json
│   │   │   └── run_metadata.json
│   │   ├── btc_1m_direction-on-move_lgbm_mcc0150_20260422/
│   │   │   ├── task_b_v2_lgbm.txt
│   │   │   └── task_b_v2_schema.json
│   │   ├── btc_1m_flowgate-v1_7layer-orderflow_lgbm_20260422/
│   │   │   ├── strategy.json
│   │   │   ├── task_a_lgbm.txt
│   │   │   ├── task_a_schema.json
│   │   │   └── task_b_lgbm.txt
│   │   ├── btc_1m_move-detect_lgbm_mcc0024_noedge_20260422/
│   │   │   ├── task_a_v2_lgbm.txt
│   │   │   └── task_a_v2_schema.json
│   │   ├── rank_checkpoints.py
│   │   └── RANKING.md
│   ├── btc_lgbm.txt
│   ├── btc_xgb.json
│   ├── feature_schema.json
│   ├── task_a_v2_lgbm.txt
│   ├── task_a_v2_schema.json
│   ├── task_b_v2_lgbm.txt
│   └── task_b_v2_schema.json
├── strategies/
│   ├── confidence_gate_v1.json
│   ├── flowgate_1m_v1.json
│   ├── mean_reversion_vwap_v1.json
│   ├── regime_switcher_ranging_v1.json
│   ├── regime_switcher_trending_v1.json
│   ├── sweep_hunter_v1.json
│   ├── trend_rider_v1.json
│   ├── vol_breakout_v1.json
│   └── vwap_sniper_v1.json
├── baselines.py
├── compare_checkpoints.py
├── config.py
├── data_loader.py
├── evaluate.py
├── features.py
├── fetch_indicators.py
├── hypothesis_v3.md
├── metrics.py
├── normalize_features.py
├── requirements.txt
├── run_tournament.py
├── targets.py
├── train.py
├── train_task_a.py
├── train_task_b.py
├── train_v2.py
└── walk_forward.py
```

To regenerate (from repo root):

```bash
python3 - <<'PY'
import os
from pathlib import Path
root = Path("python_pipeline")
def walk(dirpath: Path, prefix: str = "", is_last: bool = True, is_root: bool = False):
    if is_root:
        print(str(dirpath) + "/")
    else:
        branch = "└── " if is_last else "├── "
        print(prefix + branch + dirpath.name + ("/" if dirpath.is_dir() else ""))
    if not dirpath.is_dir():
        return
    children = sorted(dirpath.iterdir(), key=lambda p: (not p.is_dir(), p.name.lower()))
    for i, child in enumerate(children):
        last = i == len(children) - 1
        ext = "    " if is_last else "│   "
        new_prefix = prefix + ext if not is_root else ""
        walk(child, new_prefix, last, False)
walk(root, is_root=True)
PY
```
