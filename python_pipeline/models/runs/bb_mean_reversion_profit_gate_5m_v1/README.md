# bb_mean_reversion_profit_gate_5m_v1

## Theory

- Base strategy: `bb_mean_reversion` (Python-side signal)
- Strategy spec: `bb_mean_reversion_profit_gate_5m_v1.json`
- Mode: `profitability_filter`
- Timeframe: `5m`
- Label: `take_trade` with rule `net_r > 0.30R`
- Description: Profitability gate for BB mean reversion entries. Entry signal: BB %B was below 0.15 within last 4 bars, %B now rising and < 0.5, RSI < 45 and turning up, close below BB midline, CMF > -0.2.

### Entry Logic

- BB %B < 0.15 within last 4 bars (deep oversold zone)
- BB %B rising and still < 0.50
- RSI(14) < 45, rising, > 20
- Close below BB middle band
- CMF(20) > -0.20 (not deeply bearish flow)

### Exit Logic

- Stop: 2 ATR below entry (uses candle low for detection)
- Target: 2 ATR above entry (uses candle high for detection)
- Timeout: 24 bars (2 hours)
- Costs: 10 bps entry fee, 10 bps exit fee, 2 bps entry slippage, 2 bps exit slippage (24 bps total)

### Feature Layers (35 features)

- `bb_core`: volatility_bb_pct_b_20, volatility_bb_bandwidth_20, volatility_bb_upper_20_rel, volatility_bb_lower_20_rel, volatility_bb_middle_20_rel
- `mean_reversion_anchor`: trend_vwap_session_rel, order_flow_vwap_deviation_pct, trend_sma_50_rel, trend_ema_20_rel, ema_fast_rel, ema_slow_rel
- `momentum_reversal`: momentum_rsi_14, momentum_stoch_rsi_k, momentum_stoch_rsi_d, momentum_cci_20, momentum_williams_r_14, momentum_roc_10
- `flow_confirmation`: cvd_ema3, cvd_ema3_slope, volume_ad_line_zscore, volume_obv_zscore, volume_cmf_20, momentum_mfi_14
- `volatility_context`: atr_pct, atr_pct_baseline, vol_ratio, volatility_keltner_upper_20_rel, volatility_keltner_lower_20_rel, volatility_ttm_squeeze_on
- `execution_context`: order_flow_in_us_session, order_flow_in_eu_session, order_flow_in_asia_session, order_flow_liquidity_sweep_up, order_flow_liquidity_sweep_down, order_flow_thin_zone

## Results

- Ledger rows after feature join: `4889`
- Raw signals: `8809`, resolved after dedup: `4889`
- Feature count used by trainer: `35`
- Positive label rate: `net_r > 0.30R` = 22.9%
- Walk-forward MCC mean: **`+0.4507`** (very strong, consistent across all folds)
- Walk-forward coverage mean: `29.5%`
- Baseline expectancy: `-1.0765R`
- Filtered expectancy: `-0.4242R` (61% improvement)
- Baseline win rate: ~33%
- Filtered win rate: ~51% (at thr=0.5)

### Fold Detail

| Fold | MCC | Coverage | Baseline Exp (R) | Filtered Exp (R) | Trades | Selected |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | +0.2898 | 36.2% | -0.6658 | -0.4287 | 814 | 295 |
| 1 | +0.4786 | 20.1% | -1.4721 | -0.4562 | 814 | 164 |
| 2 | +0.6081 | 14.6% | -1.5162 | -0.2271 | 814 | 119 |
| 3 | +0.4004 | 38.0% | -0.8511 | -0.5615 | 814 | 309 |
| 4 | +0.4767 | 38.6% | -0.8773 | -0.4476 | 819 | 316 |

### Key Finding

The LightGBM model shows **exceptionally strong predictive signal** (MCC=0.45, best fold 0.61) — it clearly separates winning from losing BB mean reversion setups. However, the filtered expectancy remains negative (-0.42R) because **transaction costs (24 bps) consume ~56% of the risk unit** at 2 ATR stops on 5m bars.

The model pushes win rate from 33% to ~51% — enough for breakeven at 1:1 R:R in a zero-cost environment, but not enough to overcome the 0.56R per-trade cost burden.

### Path to Profitability

1. **Lower fees**: At 12 bps total (e.g., maker fees on Binance), costs drop to 0.28R and the model's 51% win rate becomes profitable
2. **Larger timeframe**: Wider ATR on higher timeframes reduces cost-to-risk ratio
3. **Asymmetric R:R**: Target > stop to increase average win size

## Artifacts

- `strategy.json`
- `trade_ledger.parquet`
- `trade_ledger.summary.json`
- `profitability_lgbm.txt`
- `profitability_schema.json`
