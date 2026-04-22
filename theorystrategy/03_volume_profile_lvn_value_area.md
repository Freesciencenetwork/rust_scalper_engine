# Volume Profile LVN Rejection / Value Area Rotation

## Thesis

Use rolling auction structure, not just time-series indicators.

You already compute value area low, point of control, and value area high. The next step is to turn volume profile into an explicit strategy family:

- value-area mean reversion
- POC magnet trades
- low-volume-node rejection
- value-area escape and acceptance

## Why This Is Attractive

- Extends existing work instead of starting from zero
- Adds market-structure features most indicator-only models miss
- Supports both reversal and expansion setups
- Useful in both ranging and transitioning regimes

## Core Setup Logic

Rotation example:

1. Price is inside value area
2. It stretches toward VAL or VAH
3. Order flow weakens at the edge
4. Model predicts rotation back toward POC or mid-value

LVN rejection example:

1. Price moves into a nearby low-volume node
2. Acceptance fails quickly
3. Momentum and flow do not support migration
4. Model predicts fast rejection back toward previous value

Acceptance / expansion example:

1. Price leaves value area
2. Stays outside long enough to show acceptance
3. Volume and momentum confirm
4. Model predicts continuation away from prior auction

## Rust Indicators Already Available

- Rolling volume profile VAL / POC / VAH
- VWAP and bands
- CVD EMA and slope
- Liquidity sweeps
- Thin zone
- Trend and momentum stack

## Missing Indicators / Features To Add

- Distance to nearest HVN
- Distance to nearest LVN
- POC drift / migration
- Time spent inside value area
- Acceptance score outside VAH / VAL
- Re-entry into value area flag
- Value area overlap with prior window
- Auction rotation factor
- Node density around current price

## Notes On Feasibility

This is very doable with your current backend because the core profile engine already exists. The main work is exposing more profile-derived features, not inventing a new data source.

## Good Training Labels

- return to POC within next N bars
- acceptance outside value area
- rejection after LVN probe

## Why I’d Prioritize It

This is the highest-leverage extension of your current volume-profile implementation and gives you a strategy family rather than one single trigger.
