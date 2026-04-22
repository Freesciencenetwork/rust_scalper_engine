# Divergence + Structure Reversal Scalp

## Thesis

Build a specialist reversal model that only activates when price makes a structural push but momentum or flow no longer confirms it.

This is not generic mean reversion. It is a setup around exhaustion plus structure:

- higher high with weaker momentum
- lower low with weaker sell pressure
- sweep into prior level followed by failed continuation

## Why This Is Attractive

- Complements trend and breakout models
- Can use your existing oscillators and CVD-style flow
- Creates a high-conviction subset instead of noisy always-on reversal trading
- Useful after sweeps, failed breakouts, and late-trend pushes

## Core Setup Logic

Bullish reversal example:

1. Price prints a fresh local low or sweep down
2. RSI / MACD / Stoch RSI or CVD fails to make a confirming low
3. Price reclaims a local trigger level
4. Model predicts short-term reversal

Bearish case is symmetric.

## Rust Indicators Already Available

- RSI
- MACD
- Stochastic and Stoch RSI
- CVD EMA and slope
- Liquidity sweep flags
- OBV / A/D / CMF
- Candlestick patterns

## Missing Indicators / Features To Add

- Swing-high / swing-low detector
- Bullish and bearish RSI divergence
- MACD divergence
- CVD divergence
- Hidden divergence flags
- Failure-swing logic
- Local structure break / reclaim flag
- Multi-signal divergence score

## Important Constraint

Divergence alone is too noisy. The strategy should require structure:

- sweep
- local support / resistance test
- reclaim / rejection
- or strong candlestick confirmation

## Good Training Labels

- reversal move after divergence-confirmed reclaim
- failed divergence vs successful divergence
- no-trade class when divergence appears without structure

## Why I’d Prioritize It

This is the best specialist reversal model to sit beside your trend, VWAP, and breakout models. It is not the first build, but it is worth having in the lineup.
