# Linear Regression Channel + Bollinger Scalp

## Thesis

Combine short-term regression structure with Bollinger stretch logic so the model can separate:

- healthy pullback inside trend
- overextension likely to mean revert
- channel break likely to expand

This is different from plain VWAP reversion because the reference is a local statistical trend channel, not a session fair-value line.

## Why This Is Attractive

- Low engineering cost
- Strong continuous features for ML
- Useful in both range and trend pullback environments
- Distinct from your current strategy set

## Core Setup Logic

Trend pullback example:

1. Positive regression slope
2. Price pulls toward lower channel or lower Bollinger area
3. Momentum stops weakening
4. Model predicts bounce in trend direction

Range fade example:

1. Regression slope is flat
2. Price tags upper or lower Bollinger area near channel edge
3. No strong breakout confirmation
4. Model predicts snapback toward channel midline

Breakout example:

1. Price exits channel with rising bandwidth and momentum
2. Model predicts expansion instead of fade

## Rust Indicators Already Available

- Linear regression slope
- Bollinger bands
- Bollinger %B
- Bollinger bandwidth
- ATR
- MACD, RSI, Stoch RSI
- Keltner, Donchian, TTM squeeze

## Missing Indicators / Features To Add

- Regression intercept
- Regression fitted value
- Upper / lower regression channel bands
- Distance to regression midline
- Distance to channel edge
- Channel width normalized by ATR
- Slope acceleration
- Breakout beyond regression channel flag

## Why It Fits The Stack

Most of the ingredients already exist. You mainly need channel geometry, then the model can learn whether a band touch is a fade or a breakout.

## Good Training Labels

- snapback to regression mid within next 3 bars
- expansion away from channel after edge touch
- direction-on-move after lower-band or upper-band interaction

## Why I’d Prioritize It

It is cheap to implement and gives you a different statistical lens from VWAP and raw momentum strategies.
