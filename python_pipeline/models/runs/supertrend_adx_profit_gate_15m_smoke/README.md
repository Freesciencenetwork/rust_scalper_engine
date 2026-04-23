# supertrend_adx_profit_gate_15m_smoke

## Theory

- Base strategy: `supertrend_adx`
- Strategy spec: `supertrend_adx_profit_gate_15m_v1.json`
- Mode: `profitability_filter`
- Timeframe: `15m`
- Label: `take_trade` with rule `net_r > 0.30R`
- Description: Profitability gate for the Rust supertrend_adx strategy. The base strategy already requires SuperTrend alignment plus ADX/DMI confirmation. This model only decides whether an already-valid trend-continuation entry is worth taking after fees and slippage.

### Why This Should Work

- It is the best trainable 15m Rust candidate screened so far: 1,387 resolved trades from 2020-01-01 to 2026-04-19, with a better raw net-R profile than donchian_breakout, ttm_squeeze_fire, or macd_trend.
- The base signal is mechanically clean: trend state plus directional-strength confirmation. That is a better substrate for a profitability gate than a vague all-bars direction model.
- The selected features answer the exact failure modes of trend-following trades: weak trend structure, fading momentum, low-quality order-flow participation, poor volatility expansion, and bad session/liquidity context.
- It keeps the layered curated-feature approach from the saved FlowGate family, but applies it to the current 15m Rust backtest path instead of old 1m research labels.

### Feature Layers

- `trend_engine`: volatility_supertrend_long, volatility_supertrend_10_3_rel, directional_adx_14, directional_di_plus, directional_di_minus, directional_psar_trend_long, directional_vortex_vi_plus_14, directional_vortex_vi_minus_14
- `trend_structure`: ema_fast_rel, ema_slow_rel, trend_ema_20_rel, trend_sma_50_rel, trend_hull_9_rel, volatility_donchian_mid_20_rel
- `momentum_confirmation`: momentum_macd_hist_norm, momentum_macd_line_norm, momentum_roc_10, momentum_tsi_25_13, momentum_awesome_oscillator_5_34_norm, momentum_rsi_14
- `flow_confirmation`: cvd_ema3, cvd_ema3_slope, order_flow_vwap_deviation_pct, volume_ad_line_zscore, volume_obv_zscore, volume_cmf_20
- `volatility_context`: atr_pct, atr_pct_baseline, vol_ratio, volatility_bb_bandwidth_20, volatility_ttm_squeeze_on, volatility_keltner_upper_20_rel, volatility_keltner_lower_20_rel
- `execution_context`: order_flow_in_us_session, order_flow_in_eu_session, order_flow_liquidity_sweep_up, order_flow_liquidity_sweep_down, order_flow_thin_zone

## Data Source

- Rust server: `http://127.0.0.1:8080`
- Backtest source: `POST /v1/strategies/supertrend_adx/backtest`
- Candle interval: `15m`
- Normalized features parquet: `/Users/francesco/Desktop/rust_scalper_engine/python_pipeline/data/features_normalized_15m.parquet`
- Date window: `2022-01-01 -> 2024-12-31`
- Warm-up days: `14`
- Costs: entry fee `10.00` bps, exit fee `10.00` bps, entry slippage `2.00` bps, exit slippage `2.00` bps
- Generated at: `2026-04-23 17:49:21 UTC`

## Results

- Ledger rows after feature join: `621`
- Raw resolved trades from backtest: `621`
- Feature count used by trainer: `38`
- Positive label rate: `net_r > 0.30R`
- Walk-forward MCC mean: `-0.0119`
- Walk-forward coverage mean: `13.0097%`
- Baseline expectancy: `-0.2384R`
- Filtered expectancy: `-0.3002R`
- Baseline profit factor: `0.6608`
- Filtered profit factor: `0.5827`
- Baseline max drawdown: `28.6654R`
- Filtered max drawdown: `4.0146R`

### Fold Detail

| Fold | MCC | Coverage | Baseline Exp (R) | Filtered Exp (R) | Baseline PF | Filtered PF | Trades | Selected |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 0.0603 | 39.8058 | -0.2202 | -0.1596 | 0.6582 | 0.7564 | 103 | 41 |
| 2 | 0.0000 | 0.0000 | -0.3978 | n/a | 0.5008 | n/a | 103 | 0 |
| 3 | 0.0000 | 0.0000 | -0.0605 | n/a | 0.9026 | n/a | 103 | 0 |
| 4 | -0.1201 | 25.2427 | -0.2155 | -0.4408 | 0.6578 | 0.4090 | 103 | 26 |
| 5 | 0.0000 | 0.0000 | -0.2982 | n/a | 0.5848 | n/a | 106 | 0 |

## Artifacts

- `strategy.json`
- `trade_ledger.parquet`
- `trade_ledger.summary.json`
- `profitability_lgbm.txt`
- `profitability_schema.json`
- `run_manifest.json`
