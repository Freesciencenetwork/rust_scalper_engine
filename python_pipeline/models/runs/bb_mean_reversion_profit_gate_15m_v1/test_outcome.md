# BB Mean Reversion 15m — Test Outcome

**STATUS (2026-04-24): DEAD — do not deploy.**

Final verdict after fold-stability audit at prob_thr=0.65 (the only configuration that produced a positive pooled mean):

- Pooled mean net_r = +0.148R is **entirely driven by 2020-2021 regime** (498 trades, +155R total).
- **Fold 1 (2021-08..2022-06): -0.838R/trade on 113 trades — catastrophic loss.**
- Fold 3 (2023-04..2024-02): 0 trades — model refuses to trade.
- Fold 4 (2024-02..2024-12): 0 trades — model refuses to trade.
- **Per-year: 2022 loses -73.5R on 89 trades; 2023 trades 2 (noise); 2024 trades 0.**

The ML filter has learned a 2020-2021 regime fingerprint that does not generalize forward. The strategy would either sit idle (2023-2024 folds) or lose catastrophically (2022). Pivoted to `macd_crossover_profit_gate_15m_v1` which shows regime-stable edge in recent years.

Historical analysis below preserved for reference.

---

## Signal Quality

Same entry rules as the 5m run (BB %B oversold within 4 bars + RSI turning up + below BB middle + CMF gate), applied to the 15m parquet over 2020-01-01 → 2024-12-31.

- 175,287 candles → 10,662 raw signals → ~3,940 non-overlapping trades (stop=3 ATR / target=3 ATR / max hold 16 bars)
- Win rate at 5bps+5bps+1bps/1bps Binance-taker fees: **48.9%**
- Gross expectancy across all fee regimes: **−0.015R** (identical entries, identical gross)

## Cost Sensitivity Scan (same signals, same ledger, four fee regimes)

| Scenario                              | Trades | WR    | Gross R  | Net R    | Cost/R | MCC (thr=0.5) | Coverage | Filtered R |
|---|---|---|---|---|---|---|---|---|
| maker-only (0 + 0 bps, 1 bps slip × 2) | 3940   | 53.4% | −0.0153  | −0.0367  | 0.017  | −0.017        | 95.8%    | −0.046R    |
| maker + taker (0 + 5 bps, 1 bps × 2)   | 3924   | 51.2% | −0.0157  | −0.0880  | 0.058  | +0.026        | 80.5%    | −0.081R    |
| taker both (5 + 5 bps, 1 bps × 2)      | 3885   | 48.9% | −0.0150  | −0.1340  | 0.099  | +0.018        | 65.9%    | −0.118R    |
| taker + extra (10 + 10 bps, 2 bps × 2) | 3628   | 43.8% | −0.0139  | −0.2220  | 0.190  | +0.050        | 20.9%    | −0.165R    |

## Buffer × Probability-Threshold Sweep (Binance taker ledger, 35 strategy features)

| buffer_r | thr  | pos%  | MCC     | cov%  | baseline exp | filtered exp | n trades |
|---|---|---|---|---|---|---|---|
| 0.00     | 0.50 | 49.5% | +0.016  | 44.7% | −0.139R      | −0.115R      | 1319     |
| 0.00     | 0.55 | 49.5% | −0.006  |  9.1% | −0.139R      | −0.116R      | 268      |
| 0.00     | 0.60 | 49.5% | +0.007  |  3.8% | −0.139R      | −0.087R      | 112      |
| 0.00     | 0.65 | 49.5% | +0.009  |  0.9% | −0.139R      | +0.013R      | **28**   |
| 0.20     | 0.50 | 41.1% | −0.001  |  1.2% | −0.139R      | −0.187R      | 35       |
| 0.50     | any  | 31.5% |  0.000  |  0.0% | −0.139R      | (none)       | 0        |
| 1.00     | any  | —     | no valid folds (starved positive class)                          |

The single positive cell (buffer=0, thr=0.65, n=28 over 5 years ≈ 6 trades/year) is too thin to call a finding.

## Conclusion — Revised after sanity checks

**Initial conclusion (premature): "signal is random".** This was wrong. Follow-up tests revealed:

### Test A — BB entry signal vs random control (16-bar forward R)

|                        | signal (n=10,658) | random (n=10,658) |
|---|---|---|
| mean                   | +0.0386R          | +0.0918R          |
| **median**             | **+0.3423R**      | +0.0417R          |
| stdev                  | 3.09R             | 3.48R             |
| **P(fwd_r > 0)**       | **56.8%**         | 50.8%             |
| P(fwd_r > +0.5R)       | 46.6%             | 40.9%             |
| P(fwd_r < −0.5R)       | 34.7%             | 39.5%             |
| Welch t-test (means)   | p = 0.24          | — (indistinguishable) |
| KS 2-sample (dist.)    | p < 0.0001        | — (distributions differ) |

The BB signal DOES carry a real directional bias: +6 pp win-rate lift vs random, median 8× higher, fewer big drawdowns, but the **mean** is compressed by heavy negative tails (σ ≈ 3R).

### Test B — Can 135 features predict forward direction at all?

Label = sign of 16-bar forward return on a 35,041-row subsample (not strategy-filtered). LightGBM walk-forward:

| Fold | MCC     | AUC    |
|---|---|---|
| 0    | +0.022  | 0.524  |
| 1    | +0.039  | 0.532  |
| 2    | +0.082  | 0.560  |
| 3    | +0.064  | 0.550  |
| 4    | +0.069  | 0.547  |
| Mean | +0.055  | **0.542** |

Features carry weak but real directional signal. Not random.

### Test C — Correct framing: label = `fwd_ret_r > 0`, train on BB-signal rows only

| label_thr | prob_thr | pos% | cov%  | n_sel | mean_fwd | median_fwd | WR%  | net (12bps) | MCC    |
|---|---|---|---|---|---|---|---|---|---|
| 0.00      | 0.50     | 56.7%| 88.1% | 7824  | +0.006R  | +0.306R    | 55.9 | −0.124R     | −0.015 |
| 0.00      | 0.60     | 56.7%| 16.3% | 1449  | +0.031R  | +0.350R    | 56.5 | −0.099R     | +0.003 |
| **0.00**  | **0.65** | 56.7%| 7.7%  | **682**| **+0.153R** | +0.422R | **58.2** | **+0.023R** | +0.012 |
| 0.30      | 0.50     | 51.0%| 81.5% | 7233  | +0.037R  | +0.351R    | 56.8 | −0.093R     | +0.027 |
| 0.50      | 0.65     | 46.6%| 0.2%  | 19    | +1.116R  | +1.306R    | 89.5 | +0.986R     | +0.030 |

**The ML filter at (label=0, prob=0.65) lifts mean forward-R from +0.04R to +0.15R — a 4× amplification, positive after 12bps taker fees (+0.023R).** Thin (~136 trades/year) but real.

### Test D — Exit design sensitivity (same BB signals, no ML filter)

| Exit design                    | n    | WR%  | Gross R  | σ    | Median   | Net (12bps) | Exit mix |
|---|---|---|---|---|---|---|---|
| A1 horizon-only H=16           | 3347 | 56.1 | +0.0005  | 3.26 | +0.323   | −0.130R     | 100% horizon |
| A2 −1R/+1R H=16                | 6157 | 47.8 | −0.0471  | 0.99 | −1.000   | −0.177R     | 52/47/1 |
| A3 −1R/+2R H=16                | 5772 | 35.0 | −0.0390  | 1.33 | −1.000   | −0.169R     | 63/26/10 |
| A4 −2R/+3R H=16 [current]      | 4454 | 47.7 | −0.0519  | 1.98 | −0.251   | −0.182R     | 44/18/38 |
| A5 −1R/+0.5R H=16              | 6926 | 63.1 | −0.0522  | 0.72 | +0.500   | −0.182R     | 63/37/0  |
| A6 −1R stop, +0.5R trail H=16  | 6910 | 65.2 | −0.0861  | 0.77 | +0.109   | −0.216R     | 65/35/0  |
| A7 −0.5R/+1R H=32              | 7146 | 32.5 | −0.0129  | 0.70 | −0.500   | −0.143R     | 68/32/0  |
| A8 BB-mid target −2R stop H=16 | 4622 | 57.4 | −0.0684  | 1.62 | +0.599   | −0.198R     | 47/37/15 |

**All 8 exit designs produce negative gross-R**, even though the underlying signal has a +0.04R mean and +0.34R median on raw horizon. Every stop/target level cuts losses that would have recovered within the horizon. Trailing stop actually makes it worse (lock-in locks out recoveries).

### Why the edge evaporates: per-trade Sharpe arithmetic

Signal edge: mean 0.039R, stdev 3.09R → **per-trade Sharpe = 0.013**.
Taker fee drag: 0.13R / 3.09R stdev = **0.042 Sharpe-units of cost**.
Ratio: fees are **3.2× the per-trade alpha Sharpe**.

No exit can fix this — it's a signal-quality-to-cost problem at the bar level.

## What actually works (first positive cells found)

Combining ML filter + low fees + horizon exit produces realistic positive expectancy:

| Configuration                                   | trades/yr | Mean fwd-R | Cost/R | Net/yr @ 1% risk |
|---|---|---|---|---|
| No filter, horizon exit, taker (12bps)          | 670       | +0.001     | 0.130  | ~ −86R / yr      |
| **ML filter (thr=0.65) + horizon + taker**      | 136       | +0.153     | 0.130  | +3.1R / yr ≈ +3%  |
| **ML filter + horizon + maker-only (2bps)**     | 136       | +0.153     | 0.020  | +18R / yr ≈ +18%  |
| Unfiltered + horizon + maker-only                | 670       | +0.001     | 0.020  | −12.7R / yr      |

Maker-only execution assumes the entry can be filled as a limit order (not guaranteed on mean-reversion setups) and the exit is at-market horizon.

## Same exercise, other 15m entries (from session transcript)

| Strategy            | Trades | Gross R  | Net R    |
|---|---|---|---|
| macd_crossover      | 5,062  | +0.010   | −0.15    |
| rsi_pullback        | 2,452  | +0.005   | −0.17    |
| donchian_breakout   |   714  | +0.009   | −0.08    |
| supertrend_adx      |   757  | −0.030   | −0.22    |
| bb_mean_reversion   | 3,940  | −0.015   | −0.09 … −0.22 (fee-dependent) |

None exceed gross +0.01R. ML training on each showed MCC ≈ 0 with near-zero coverage.

## Decision

There IS a real but thin edge on BB mean-reversion 15m. It survives ML filtering at prob_thr=0.65 (n=136/yr, mean +0.153R, +6pp WR lift, net +0.023R at taker fees). Taker fees consume ~85% of the filtered edge; maker-only execution would multiply net yield ~8×.

**Do not abandon; requalify.** The question is no longer "does this have edge?" but "can execution realistically capture the thin +0.023R/trade edge?"

## Prioritized next experiments

1. **Verify maker-fill feasibility.** Current simulation assumes entry-at-close; real maker fills on mean-reversion setups (buy at pullback low) need a concrete limit-order fill model. Test: place limit at entry_price and check if `low[idx+1] ≤ limit` within k bars — this gives an unbiased estimate of fill rate.
2. **Stress-test the prob_thr=0.65 cell.** 682 trades over 5 years is ~136/yr — thin. Need fold-level stability check (not just pooled metrics) and per-year expectancy to confirm it's not concentrated in a single regime (e.g., 2020 bull only).
3. **Higher TF (1h/4h) for comparison.** Regenerate ledgers via Rust HTTP API. At 1h: cost/R drops ~0.03R, so the unfiltered signal (mean +0.04R horizon) may already be net-positive without ML filter.
4. **Deploy decision.** If (1) maker fills are feasible and (2) fold-stability holds, this becomes the first viable run. If (1) fails, move to higher TF via (3).

## Directions explicitly de-prioritized

- **Adding more TA indicators.** 135 is already enough; AUC 0.54 on raw direction says the constraint is signal density, not feature count.
- **Continuing to sweep stop/target parameters.** 8 designs tested, all negative gross; the signal's variance is too high relative to target move for any intraframe exit to extract edge.
- **Inverse-entry sanity check.** Dropped — Test A already confirmed the signal is informative (KS p<0.0001, +6pp WR). Inverting would be near-random too.
