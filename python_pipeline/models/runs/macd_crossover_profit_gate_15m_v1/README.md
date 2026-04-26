# macd_crossover_profit_gate_15m_v1

**Status**: Conditional deploy candidate. Fee-sensitive. Requires maker-entry execution.

## Signal

MACD histogram zero-cross long on 15m BTC-USDT:

- `momentum_macd_hist_norm[t] > 0` and `momentum_macd_hist_norm[t-1] <= 0`

~6,936 raw signals on 2020-01-01 → 2024-12-31 universe (one per bar crossover).

## Label & ML filter

- **Label**: `fwd_ret_r > 0` at a 16-bar (4-hour) horizon. `fwd_ret_r = (close[t+16] - close[t]) / atr[t]`.
- **Features**: 129 normalized indicator features (momentum, trend, volume, volatility, flow, regime).
- **Model**: LightGBM binary classifier, walk-forward with 5 expanding folds (val_ratio=0.15).
- **Selection**: `proba >= 0.55` (prob_thr).

## Exit

- **Horizon close only** (exit at `close[t+16]`). Intra-hold stop/target exits were tested and strictly lose money (per-trade Sharpe ~0.05 cannot survive 2ATR stops — see `test_outcome.md`).

## Execution assumption

- **Entry**: maker limit at signal close (0 bps fee, 1 bps slip budget). Fill-rate on momentum-long limits: see test_outcome.md.
- **Exit**: taker at horizon close (5 bps fee, 1 bps slip). Total RT cost ≈ 7 bps.

## Headline results (5 walk-forward folds, 2020-2024)

| Scenario | pooled mean net_r | pos folds | pos years | notes |
|---|---|---|---|---|
| 7 bps RT (maker+taker) | **+0.153 R** | 3/4 active | 4/5 | 2024 OOS fold +0.45R strongest |
| 5 bps RT | +0.148 R | 3/4 | 4/5 | borderline comfortable |
| 12 bps RT (full taker) | -0.055 R | 2/4 | 1/5 | unprofitable |

Selected trades: ~134/year at `thr=0.55` (total 668 across 5 years).

## Files

- `strategy.json` — strategy spec (symlinked from `python_pipeline/strategies/`)
- `trade_ledger.parquet` — 6,934 rows, one per raw MACD signal (allow_overlap=True, max_cost_r=99 so no cost filter)
- `trade_ledger.summary.json` — basic ledger stats
- `profitability_lgbm.txt` — final LightGBM model trained on the full pre-test window
- `profitability_schema.json` — feature schema and model metadata
- `test_outcome.md` — full test log including cost-correction saga

## Reproduce

```bash
PYTHONPATH=. python3 -m python_pipeline.training.build_signal_ledger \
    --strategy macd_crossover \
    --indicators python_pipeline/data/indicators_full_15m.parquet \
    --features python_pipeline/data/features_normalized_15m.parquet \
    --from-date 2020-01-01 --to-date 2024-12-31 \
    --max-hold-bars 16 \
    --entry-fee-bps 0 --exit-fee-bps 5 \
    --entry-slippage-bps 1 --exit-slippage-bps 1 \
    --max-cost-r 99 --use-forward-returns --allow-overlap \
    --output python_pipeline/models/runs/macd_crossover_profit_gate_15m_v1/trade_ledger.parquet

PYTHONPATH=. python3 -m python_pipeline.training.train_profitability_filter \
    --strategy macd_crossover_profit_gate_15m_v1 \
    --data python_pipeline/models/runs/macd_crossover_profit_gate_15m_v1/trade_ledger.parquet \
    --label-mode fwd_ret_r --buffer-r 0.0 --prob-thr 0.55 \
    --output-dir python_pipeline/models/runs/macd_crossover_profit_gate_15m_v1
```

## Known limitations

1. **Fee-brittle**: +0.09R at 7bps RT shrinks to -0.05R at 12bps RT (taker-only) — execution quality is the margin of safety.
2. **Single-market**: BTC-USDT only. Cross-asset generalization unverified.
3. **Fold 2 refusal**: In the 2022-07..2023-05 transition window the model declined to trade (coverage 0%). Live behaviour in similar regimes is uncertain — model may stop trading for extended periods.
4. **High variance**: σ(net_r) ≈ 3.3R per trade. Position sizing must assume tail-heavy distribution.
5. **Small 2024 sample (fold 4)**: 73 selected trades in 2024 with +0.45R mean is encouraging but not statistically robust alone.

## Deployment next steps

Before going live:

1. Paper-trade the model for ≥1 month at maker-only execution to verify fill-rate and realised expectancy.
2. Compare realised cost per trade to the 7bps assumption.
3. Implement kill-switch if rolling 30-trade mean_net drops below -0.3R (2 sigma from pooled mean).
4. Consider applying to 1h/4h timeframes where close/atr is smaller → lower cost/R.
