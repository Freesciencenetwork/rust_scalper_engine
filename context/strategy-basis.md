# What this engine’s strategy is based on

## One-line label

**Long-only BTC, 15m bar close, bullish continuation pullback** — arm a **buy stop** above the signal bar only when structure, flow, volatility, and calendar vetoes all agree.

## Market idea (hypothesis)

- Continuation works better when **1h** and **15m** trends already agree (fast EMA above slow).
- Good entries look like **pullback into fast support + reclaim by the close**, not chasing green candles.
- **Cumulative delta / CVD** slope should still support buyers (order-flow proxy).
- **VWMA(96)** anchors “value”; the model wants price **above** that context for longs.
- **Adverse regimes** (macro windows, weekends, volatility spikes, failed breakouts, compressed ATR) → stand aside.

Source: `README.md` “Theory of the engine”; implementation: [`strategies/default`](../src/strategies/default/) (`StrategyEngine` + gates). Extra TA (RSI, MACD, …) is computed into [`PreparedCandle.indicator_snapshot`](../src/market_data/snapshot.rs) but **not** used by the default strategy gates.

## Technical building blocks

| Building block | Role |
|----------------|------|
| **15m OHLCV + delta** | Base series; delta → CVD → EMA(3) slope (“flow”) |
| **EMA 9 / 21** | Trend on 15m; same EMA pair on **aggregated 1h** |
| **ATR(14)** | Stop/target distance; ATR% vs rolling median → high-vol regime |
| **VWMA(96)** | Medium-horizon context filter (`close > VWMA`) |
| **Swing highs (5-bar pivot)** | “Runway” — veto if resistance too close above entry |
| **Volume profile (rolling)** | POC / VAH / VAL from OHLCV; longs vetoed if `close < VAL` when `vp_enabled` |
| **Breakout / failed acceptance** | Stateful: close above N-bar high then failure → block until reclaimed |
| **Macro events + weekend ban** | Hard vetoes on time |
| **Tick / lot rounding** | Venue-style plan: trigger = high + tick; stop/target from ATR multiples |

## Entry expression (when `ArmLongStop`)

- **Trigger**: buy-stop one tick above **signal bar high** (`buy_stop_trigger_price`).
- **Plan**: stop ≈ trigger − `stop_atr_multiple × ATR`, target ≈ trigger + `target_atr_multiple × ATR`, size from `risk_fraction` and rounded steps.

Not based on: ML prediction of next candle, order-book L2, funding, cross-asset signals, or short/mean-reversion logic.

## Design philosophy

- **Confluence over frequency**: many independent AND gates; first failing reason recorded.
- **Risk from volatility**: ATR-scaled geometry, not fixed dollar stops.
- **Skip > guess** during shocks, weekends, high vol ratio, failed acceptance, or insufficient history (large lookbacks, default ~960 bars for vol baseline).

## Optional overlay

**RustyFish** numeric overlay can tighten parameters (e.g. vol floor) when external daily report is attached — policy in `src/context/`.
