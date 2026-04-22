# Checkpoint Evaluation — 10 Random Windows

Generated 2026-04-22 10:41.  Each window = 10,000 consecutive 1m bars (~6d).  Windows drawn stratified by year.

## Per-window results

| Window (UTC)         | down-flat-up_lgbm_acc68pct_noedge_20260421 | 1m_direction-on-move_lgbm_mcc0150_20260422 | flowgate-v1_7layer-orderflow_lgbm_20260422 | m_move-detect_lgbm_mcc0024_noedge_20260422 |
| -------------------- | ------------- | ------------- | ------------- | ------------- |
| 2012-02-13 → 2012-03-18 | skip (few bars) | +0.078/57.89% | +0.262/80.53% | +0.000/26.44% |
| 2013-10-10 → 2013-11-21 | +0.121/56.77% | +0.150/57.28% | +0.196/59.32% | -0.041/52.66% |
| 2014-08-27 → 2014-10-08 | +0.132/56.68% | -0.040/46.67% | +0.138/56.87% | +0.000/28.85% |
| 2016-06-07 → 2016-07-19 | +0.168/58.96% | +0.387/70.83% | +0.127/56.43% | +0.000/45.38% |
| 2017-11-10 → 2017-12-22 | +0.072/53.66% | +0.060/53.18% | +0.062/60.10% | -0.001/59.99% |
| 2019-09-12 → 2019-10-24 | +0.137/56.79% | skip (few bars) | +0.159/58.08% | skip (few bars) |
| 2020-03-14 → 2020-04-25 | +0.136/56.80% | skip (few bars) | +0.119/56.94% | skip (few bars) |
| 2022-07-12 → 2022-08-22 | +0.083/54.19% | skip (few bars) | +0.080/56.21% | skip (few bars) |
| 2024-09-26 → 2024-11-06 | +0.031/51.72% | skip (few bars) | +0.078/54.00% | skip (few bars) |
| 2025-10-06 → 2025-11-16 | +0.011/50.01% | skip (few bars) | +0.064/57.24% | skip (few bars) |
| -------------------- | ------------- | ------------- | ------------- | ------------- |
| MEAN ± STD           | +0.099±0.050  | +0.127±0.144  | +0.129±0.061  | -0.008±0.016  |

## Aggregate ranking (by mean MCC)

| # | Checkpoint | Mean MCC | ±Std | Mean Acc |
| - | ---------- | -------- | ---- | -------- |
| 1 | btc_1m_flowgate-v1_7layer-orderflow_lgbm_20260422 | +0.1286 | ±0.0609 | 59.6% | YES |
| 2 | btc_1m_direction-on-move_lgbm_mcc0150_20260422 | +0.1272 | ±0.1436 | 57.2% | YES |
| 3 | btc_1m_3class_down-flat-up_lgbm_acc68pct_noedge_20260421 | +0.0989 | ±0.0499 | 55.1% | MARGINAL |
| 4 | btc_1m_move-detect_lgbm_mcc0024_noedge_20260422 | -0.0083 | ±0.0162 | 42.7% | NO |

---
*Run `python3 evaluate_checkpoints.py` to refresh.*
