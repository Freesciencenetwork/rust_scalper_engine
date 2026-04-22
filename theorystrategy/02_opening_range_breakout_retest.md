# Opening Range Breakout / Retest Scalp

## Thesis

Train a model only around the first structured range of a major session, then ask whether the breakout is likely to expand, fail, or retest and continue.

For crypto, this is still useful even without an exchange open because EU and US session transitions create repeatable shifts in participation and volatility.

## Why This Is Attractive

- Very structured event set
- Reduces training on low-information random minutes
- Naturally aligned with session effects you already track
- Easy to pair with volume, VWAP, and momentum confirmation
- Clean for both classification and selective trading

## Core Setup Logic

Bullish example:

1. Define opening range for a chosen session window
2. Capture opening-range high, low, mid, and width
3. Price breaks above the range
4. Confirmation comes from volume, momentum, and trend context
5. Model predicts whether breakout holds for the next few bars

Important variant:

- breakout
- pullback to range high
- hold above it
- continuation

This retest version is often higher quality than first-touch breakout chasing.

## Rust Indicators Already Available

- Session flags: Asia / EU / US
- Session VWAP and bands
- ATR and ATR baseline
- Bollinger bandwidth
- TTM squeeze
- ADX / DI
- SuperTrend / PSAR
- CVD EMA and slope
- OBV / A/D / CMF

## Missing Indicators / Features To Add

- Opening-range high / low / mid
- Opening-range width
- Width normalized by ATR
- Breakout distance from range
- Breakout persistence count
- Retest-hold / retest-fail flags
- Time since session open
- Relative volume by minute-of-session
- Session-specific range templates

## Minimum Practical Session Definitions

- EU open window
- US open window
- EU-US overlap window

You do not need stock-style market opens. You need consistent intraday participation regimes.

## Good Training Labels

- breakout follow-through over next 3 to 5 bars
- failed breakout vs accepted breakout
- direction-on-move only after breakout confirmation

## Why I’d Prioritize It

This gives you a high-signal event-driven dataset and should combine well with your current regime and session features.
