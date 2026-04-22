# Training Hypothesis — v3
**Date:** 2026-04-22
**Timeframe:** 1m candles only
**Data:** BTC/USD 2012–2026 (~1.27M rows after feature merge)

---

## What we are running

Two sequential binary classifiers, both on 1-minute BTC/USD candles:

**Task A — MOVE detector**
Can the model predict whether the next 5 bars will produce a price move
larger than 1× the recent realized volatility?
- Features: 26 curated indicators (order flow, VWAP, regime, session, momentum)
- Horizon: 5 bars (5 minutes)
- Target: `is_move = 1` if `|future_return| > 1.0 × rolling_vol_60`

**Task B — DIRECTION classifier**
Given that we are on a MOVE bar, can the model predict UP vs DOWN?
- Same 26 features, same 1m data — no resampling
- Horizon: 3 bars (3 minutes)
- Target: `direction = 1 (UP)` or `0 (DOWN)` on MOVE bars only
- Validation: expanding walk-forward, 5 folds

---

## The hypothesis

**H1 — Task A:**
The 26 curated indicators contain enough regime and volatility context
to detect when a significant price move is imminent at the 1-minute level.
Specifically, the combination of TTM Squeeze (compression → breakout),
ATR% (volatility normalization), and ADX (trend strength) should identify
high-probability MOVE environments better than a volatility-threshold baseline.

**H2 — Task B:**
On bars already identified as significant moves, the direction is not random.
CVD slope (buyer/seller aggression), VWAP deviation (mean-reversion pressure),
liquidity sweep flags (institutional stop-hunt patterns), and session context
(EU/US overlap vs Asia) carry enough directional information to predict UP vs
DOWN better than momentum continuation baselines.

The prior run (v2, Fold 0) showed MCC = +0.27 on 15m MOVE bars. We expect
similar or better on 1m because more data is available, features are native
to 1m resolution, and no information is lost through resampling.

---

## What we expect

| Task | Expected MCC | Confidence | Why |
|---|---|---|---|
| Task A | 0.05 – 0.12 | Low–Medium | v2 got 0.02 with 129 noisy features; curated 26 should improve but MOVE detection on 1m is hard |
| Task B | 0.20 – 0.30 | Medium | v2 Fold 0 on 15m was 0.27; 1m has more data and native features |

**Expected accuracy range (Task B):** 60–65%
**Expected directional return separation:** predicted UP bars should average
+0.10% to +0.15% future return; predicted DOWN bars should average -0.10% to -0.15%.

---

## Baseline for success

The model must beat ALL trivial baselines on MCC across ALL folds.
Baselines:
- `always_move` / `always_nomove` (Task A)
- `vol_threshold` — predict MOVE when realized vol > training median (Task A)
- `always_up` / `always_down` (Task B)
- `prev_return_sign` — momentum continuation (Task B)
- `rolling_momentum` — ROC-10 sign (Task B)

**Minimum bar to claim real signal:**
- MCC > 0.10 on Task A (weak but real)
- MCC > 0.20 on Task B (usable signal, consistent across 5 folds)
- Average future return by predicted class must show meaningful separation
  (predicted UP > 0, predicted DOWN < 0)
- Result must be consistent across folds — one lucky fold doesn't count

**Stretch goal:** Task B MCC > 0.30 across all 5 folds — that would be
a tradeable directional edge worth building execution logic around.

---

## If we fail

- Task A MCC ≤ baseline → move detection on 1m is not feasible with OHLCV
  indicators alone. Next step: add funding rate and open interest as features.
- Task B MCC ≤ baseline → direction on 1m is pure noise even on MOVE bars.
  Next step: widen horizon to 10–15 bars, or switch to regime-conditional models.
- MCC inconsistent across folds → overfitting or regime shift. Next step:
  restrict training data to post-2018 (modern BTC market microstructure).
