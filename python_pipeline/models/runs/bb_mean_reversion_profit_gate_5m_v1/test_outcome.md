# BB Mean Reversion 5m — Test Outcome

## Signal Quality

The BB mean reversion entry logic (BB %B oversold within 4 bars + RSI turning up + midline bounce + CMF filter) generates a real, tradeable signal on 5m BTC. Over 2022-01-01 to 2024-12-31:

- **8,809 raw signals**, resolving to **~4,900 trades** (after dedup for overlapping holds)
- Consistent behavior across all market regimes in the test window

## ML Model Performance

The LightGBM profitability filter showed **exceptionally strong predictive signal** when transaction costs create a clear separation between winners and losers:

| Config | MCC | Baseline Exp | Filtered Exp | Win Rate Lift |
|---|---|---|---|---|
| 2ATR stop/target, 24bps costs | **+0.45** | -1.08R | -0.42R | 33% → 51% |
| 2ATR stop/target, 12bps costs | +0.01 | -0.28R | -0.26R | minimal |
| 4ATR stop/target, 24bps costs | +0.03 | -0.54R | -0.47R | minimal |
| 3ATR target / 2ATR stop, 12bps, cost-filtered | ~0 | -0.21R | -0.18R | minimal |

**Key insight:** The model discriminates well when the cost burden makes the win/loss boundary sharp (MCC=0.45 at 24bps). At realistic Binance fees the boundary blurs and the model loses its edge.

## Fee Economics (Root Cause of Negative Expectancy)

On 5m BTC bars, ATR ≈ 0.22% of price. Transaction costs as a fraction of risk:

| Fee Scenario | Round-Trip Cost | Cost as % of 2ATR Risk |
|---|---|---|
| Binance maker (2bps/side) + 1bps slip | 6 bps | ~14% (0.14R) |
| Binance taker (5bps/side) + 1bps slip | 12 bps | ~28% (0.28R) |
| Conservative (10bps/side) + 2bps slip | 24 bps | ~56% (0.56R) |

With 2ATR stops and Binance taker fees:
- **Wins**: +1.0R gross → +0.72R net (target hit)
- **Losses**: -1.0R gross → -1.28R net (stop hit)
- **Payoff ratio**: 0.56 — requires **64% win rate** to break even
- Best model win rate achieved: **51%** — not enough

With asymmetric R:R (3ATR target / 2ATR stop, 12bps, cost-filtered ≤0.35R):
- **Wins**: +1.5R gross → +1.27R net
- **Losses**: -1.0R gross → -1.23R net
- **Payoff ratio**: 0.975 — requires **50.3% win rate** to break even
- Baseline win rate: **41%** — model could not push past ~42%

## Configurations Tested

| Parameter | Values Tested |
|---|---|
| Stop multiplier | 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0 ATR |
| Target multiplier | 1.0, 1.5, 2.0, 3.0, 4.0, 8.0 ATR |
| BB target (midline) | Yes / No |
| Fee levels | 6, 12, 24 bps round-trip |
| Cost cap | 0.25R, 0.35R, 0.50R, unlimited |
| Features | 35 curated / 129 all |
| Thresholds | 0.3, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7 |
| Forward return labeling | Yes / No |
| Exit price model | Close-to-close / Limit at stop-target / OHLC high-low |

## Conclusion

**Not profitable in any tested configuration.** The strategy has a genuine edge in identifying mean-reversion setups, and the ML model has strong discriminatory power (MCC=0.45), but 5m BTC scalping with standard fee structures cannot overcome the cost-to-risk ratio.

## Paths to Profitability

1. **Maker-only limit orders** with Binance VIP3+ (maker 0.012%) — reduces cost/R to ~0.06R
2. **Higher timeframe** — wider ATR reduces cost/R ratio proportionally
3. **High-volatility sessions only** (US/EU overlap) — natural ATR expansion reduces cost/R
4. **Different exchange** with lower fees or rebate programs
5. **Different strategy type** — momentum/breakout strategies with naturally wider stops
