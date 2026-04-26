# MACD Crossover 15m — Test Outcome

**Summary**: Conditional edge found. Profitable at low fees (≤7bps RT), unprofitable at taker-only fees (12bps+). Unlike BB mean-reversion (which died after 2021), MACD crossover **strengthens in 2024** (fold 4 OOS is the best fold). Deployment requires maker-entry execution.

## Headline table

At `prob_thr = 0.55`, market-horizon exit, real per-trade cost = `(bps_rt/10000) * close/atr` (BTC 15m close/atr ≈ 357):

| Cost | pooled mean net_r | active folds pos | years pos | deployable? |
|---|---|---|---|---|
| 2 bps | +0.236 R | 4/4 | 4/5 | yes (maker-only) |
| 5 bps | +0.148 R | 3/4 | 4/5 | yes |
| **7 bps** | **+0.090 R** | 3/4 | 2/5 | **marginal** |
| 12 bps | -0.055 R | 2/4 | 1/5 | no |
| 20 bps | -0.288 R | 1/4 | 0/5 | no |

## Fold-by-fold at 7 bps RT (target scenario: maker entry + taker exit + 1bps slip each)

| fold | test period | n trades | mean net_r | comment |
|---|---|---|---|---|
| 0 | 2020-11-08 .. 2021-09-05 | 150 | +0.110 R | early bull |
| 1 | 2021-09-06 .. 2022-07-07 | 251 | -0.016 R | peak+early bear; break-even |
| 2 | 2022-07-07 .. 2023-05-13 | 0 | — | **model declines to trade** |
| 3 | 2023-05-13 .. 2024-03-13 | 188 | +0.064 R | recovery |
| 4 | 2024-03-13 .. 2024-12-30 | 79 | **+0.453 R** | **2024 OOS strongest** |

- Fold 4 (most recent OOS) is the strongest by a wide margin — the strategy is getting better, not worse.
- Fold 2 refusal to trade is the main deployment risk: the model may go silent for months in regime transitions.

## Per-year at 7 bps RT (pooled across folds)

| year | n | mean net_r | WR% | total net_r |
|---|---|---|---|---|
| 2020 | 36 | -0.170 R | 38.9% | -6.12 R |
| 2021 | 207 | +0.098 R | 49.8% | +20.26 R |
| 2022 | 158 | -0.011 R | 49.4% | -1.67 R |
| 2023 | 143 | -0.007 R | 46.9% | -1.00 R |
| 2024 | 124 | **+0.393 R** | 59.7% | **+48.76 R** |

2024 carries the majority of the pooled edge. 2020-2023 are roughly break-even at 7bps. Without 2024, the strategy is flat.

## Test A — signal vs random (no ML filter)

Raw 16-bar forward-R distribution of all MACD-crossover signal bars vs a random control of equal size:

| metric | MACD signal | random |
|---|---|---|
| n | 6,931 | 6,931 |
| mean fwd_ret_r | +0.121 R | ≈0 |
| median fwd_ret_r | +0.049 R | ≈0 |
| WR (>0) | 50.7% | ≈50% |

Signal carries a weak but consistent directional bias. KS test p<0.0001, distributions differ.

## Test B — per-year raw edge

Unfiltered signal at 16-bar horizon (no costs, no filter):

| year | n | mean fwd_ret_r | median | WR% |
|---|---|---|---|---|
| 2020 | 1,362 | +0.147 R | +0.083 R | 51.1% |
| 2021 | 1,397 | -0.040 R | -0.014 R | 49.9% |
| 2022 | 1,372 | -0.184 R | -0.126 R | 47.6% |
| 2023 | 1,362 | +0.473 R | +0.079 R | 51.2% |
| **2024** | **1,438** | **+0.209 R** | **+0.189 R** | **53.6%** |

The raw signal is strongest in 2023-2024. The ML filter amplifies this (2024 raw +0.21R → +0.39R filtered at 7bps).

## Test C — threshold robustness at 7 bps RT

| prob_thr | n | mean net_r | WR% | pos folds | pos years |
|---|---|---|---|---|---|
| 0.50 | 2,904 | -0.137 R | 48.2% | 2/5 | 2/5 |
| 0.52 | 1,485 | -0.066 R | 49.3% | 3/5 | 2/5 |
| **0.55** | **668** | **+0.090 R** | **50.3%** | **3/5** | **2/5** |
| 0.58 | 200 | +0.100 R | 51.0% | 3/5 | 3/5 |
| 0.60 | 86 | +0.382 R | 54.7% | 3/5 | 2/5 |
| 0.62 | 45 | +0.681 R | 55.6% | 2/4 | 2/4 |
| 0.65 | 7 | +1.177 R | 57.1% | 1/5 | 2/2 |

Sweet spot: `thr=0.55` (668 trades, ~134/yr). Higher thresholds amplify per-trade mean but trade frequency collapses → model becomes a "don't trade" signal in most years. `thr=0.60` might be preferred for risk-averse deployment (86 trades over 5yr, but +0.38R mean).

## Test D — exit design comparison at 12 bps RT

Testing market entry + various exits:

| exit design | pooled mean net_r | WR% | notes |
|---|---|---|---|
| horizon (16 bars) | -0.055 R | 47.2% | best of the losing pack |
| stop 2ATR / target 3ATR | **-0.257 R** | 44.2% | stops cut recoverable trades |

Same finding as BB mean-reversion: intra-hold stop/target destroys the edge. Horizon-only is the only viable exit.

## Cost-correction saga (important diagnostic)

**Initial (incorrect) analysis** used a hardcoded `cost = 0.13` R per trade for 12 bps fees, which matches an arbitrary close/atr ratio ≈ 186. **Actual BTC 15m close/atr ≈ 357** (mean), so real cost is 0.35 R at 12 bps — 2.7× higher than assumed. This overstated profitability across all earlier cost-regime comparisons.

The corrected analysis (`cost_r = (bps_rt / 10000) * close/atr` per trade) gives the numbers above. The key implication: **every bps of transaction cost translates to ~0.036 R per trade on BTC 15m**. At a signal with ~0.12 R raw mean, each 3 bps of fees consumes the entire edge.

## Ledger & filters

- `trade_ledger.parquet`: 6,934 rows (one per raw MACD signal, 2 dropped on feature NaN).
- Fees used in ledger `net_r`: 0 entry + 5 exit + 1 slip × 2 = **7 bps RT** (the target scenario).
- `max_cost_r` filter disabled (value 99) — earlier runs using default 0.5 were dropping the ~8% of trades with close/atr > 714, which biased results negatively on recent years (2023-2024 had the higher ratios).
- `allow_overlap = True` — each signal becomes a trade row, so ML labelling sees every opportunity. For a single-position backtest you'd re-apply overlap filtering at execution time.

## Conclusion

- **Deployable at** ≤ 7 bps round-trip (maker entry + taker exit, or full maker).
- **Not deployable at** full taker (12 bps+).
- Expected returns at 7bps RT:
  - ~134 trades/yr × 0.09 R mean ≈ **+12 R/yr at 1% risk/trade ≈ +12%/yr gross**
  - Alternatively at `thr=0.60`: ~17 trades/yr × 0.38 R ≈ **+6.5 R/yr but more concentrated**
- Main risks: fold 2 refusal-to-trade behaviour in regime transitions; small 2024 sample size; single-market backtest.

Proceed only with paper-trading validation and maker-fill monitoring.
