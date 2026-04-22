# Anchored VWAP Retest / Reclaim Scalp

## Thesis

Train a scalp model around the idea that after a meaningful event, price often reacts to the volume-weighted fair value from that event more cleanly than to session VWAP alone.

This is strongest after:

- breakout candles
- sweep reversals
- session opens
- local swing highs / lows
- macro impulse candles

The model should learn whether a retest of anchored VWAP is likely to hold and continue, reject and reverse, or chop.

## Why This Is Attractive

- Strong structure, not just oscillator noise
- Reuses your existing VWAP, momentum, and order-flow stack
- Works for both continuation and reversal
- Easy to define event-driven training subsets
- Good fit for tree models because distance-to-anchor features are continuous and interpretable

## Core Setup Logic

Long continuation example:

1. Strong impulse up or breakout event
2. Anchor VWAP at the event candle
3. Price pulls back into anchored VWAP or 1sd band
4. Momentum stabilizes and order flow stops deteriorating
5. Model predicts reclaim / continuation over the next 3 to 5 bars

Short continuation is the symmetric case.

Reversal variant:

1. Price fails at an anchored VWAP from a prior high
2. Rejection aligns with momentum weakness or sell aggression
3. Model predicts move away from anchored fair value

## Rust Indicators Already Available

- Session VWAP and VWAP bands
- RSI, MACD, Stoch RSI, CCI, Williams %R
- ADX, DI+/DI-, SuperTrend, PSAR
- OBV, A/D line, CMF
- CVD EMA and CVD slope
- Liquidity sweep flags
- Thin zone flag
- Session flags

## Missing Indicators / Features To Add

- Anchored VWAP from arbitrary event index
- Anchored VWAP 1sd / 2sd bands
- Distance of price from anchored VWAP
- Anchored VWAP slope
- Time since anchor
- Anchor type enum
- Multi-anchor confluence count
- Retest flag
- Reclaim / reject boolean features

## Candidate Anchor Rules

- breakout candle close above recent high
- liquidity sweep candle
- highest-volume candle in rolling window
- local swing low / swing high
- start of EU or US session

## Good Training Labels

- move detect over next 3 to 5 bars
- direction-on-move after first anchored retest
- no-trade class if price remains inside a small anchored VWAP band

## Why I’d Prioritize It

This is the cleanest next strategy because it adds genuinely new market structure while staying close to your current data and feature stack.
