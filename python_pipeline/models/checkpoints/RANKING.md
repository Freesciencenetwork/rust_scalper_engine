# Checkpoint Ranking

Sorted by MCC (descending). Generated 2026-04-22 10:36.

| #    | Checkpoint                                               | Task                                      | Timeframe | Horizon | Feats | MCC     | Acc     | Edge                                     |
| ---- | -------------------------------------------------------- | ----------------------------------------- | --------- | ------- | ----- | ------- | ------- | ---------------------------------------- |
| 1    | btc_1m_direction-on-move_lgbm_mcc0150_20260422           | Task B — Direction (UP/DOWN on MOVE bars) | 15min     | 3b      | 129   | +0.1501 |         | YES                                      |
| 2    | btc_1m_flowgate-v1_7layer-orderflow_lgbm_20260422        | Task A — MOVE detect                      | 1m        | 5b      | 30    | +0.0660 |         | MARGINAL                                 |
| 3    | btc_1m_move-detect_lgbm_mcc0024_noedge_20260422          | Task A — MOVE detect                      | 1m        | 5b      | 129   | +0.0237 |         | MARGINAL                                 |
| 4    | btc_1m_3class_down-flat-up_lgbm_acc68pct_noedge_20260421 | 3-class (DOWN / FLAT / UP)                | 1m        | 5b      | 15    | n/a     | 67.8%   | NO (accuracy misleading: FLAT dominates) |

---
Historical snapshot of checkpoint ranking data.
