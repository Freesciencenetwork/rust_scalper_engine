# Strategy Overview

This note is a consolidated retrospective across the model runs, context notes, and per-run READMEs. It is intentionally split into:

- **What went good**
- **What went wrong**

The goal is not to repeat every test number, but to preserve the reasons the workflow learned something useful and the reasons it still did not break even.

## What Went Good

### 1. The pipeline itself became usable

- The Rust-backed workflow is now coherent: feature ingest, normalization, ledger export, walk-forward training, and run-folder reporting all work as one chain.
- The run folders and their READMEs are now the main durable record of each experiment.
- The docs converged on a clean rule: keep durable pipeline code, and delete one-off scripts after use.

### 2. The work found at least one real conditional edge

- `macd_crossover_profit_gate_15m_v1` is the strongest result so far.
- At the target execution assumption of about **7 bps round-trip**, it stayed positive on pooled mean net return.
- It also held up in the most recent OOS fold, which matters more than the pooled average.
- This is the first run that looks like a genuine deploy candidate rather than a pure research artifact.

### 3. The model can separate signal from noise in some regimes

- The BB 5m run showed very strong MCC and improved win rate, which means the classifier learned something real.
- The BB 15m sanity checks showed the signal was not random: raw forward returns were better than a random control, even though the exit model destroyed that edge.
- The 129-feature walk-forward audit showed a weak but real feature signal on BTC 15m. That sets a ceiling, but it also confirms the feature set is not pure noise.

### 4. The revised framing was correct

- The biggest improvement in the analysis was switching away from “did the simulated exit make money?” as the only label.
- The docs show that a bad stop/target model can erase a real signal.
- Using forward-return style labels exposed the underlying edge more clearly and gave a better basis for the profitability filter.

### 5. Cross-cutting lessons became clear

- Per-fold and per-year reporting is necessary.
- Walk-forward validation is necessary.
- Horizon-only exits are often better than stop/target exits on thin BTC 15m edges.
- Cost math has to be explicit in R units, or the results become misleading.

## What Went Wrong

### 1. Transaction costs were too large for the edge

- This is the main reason the portfolio did not break even.
- Many strategies had a thin statistical edge, but the fee/slippage burden was larger than the signal.
- On 15m BTC, even a small round-trip cost translates into a meaningful fraction of an R unit.
- The BB 5m run was the clearest case: the filter improved classification metrics, but the cost-to-signal ratio still kept expectancy negative.

### 2. Stop/target exits destroyed recoverable trades

- Several runs showed that intraframe stop/target logic cut off trades that would have recovered within the horizon.
- That especially hurt BB-style entries.
- The docs consistently show that horizon-only exits preserved more of the signal than stop/target exits.
- Trailing stops were worse than fixed stops in the tested BTC 15m setting.

### 3. The edge was regime-dependent

- `bb_mean_reversion_profit_gate_15m_v1` looked good only in the 2020-2021 bull regime.
- Once the market changed, it lost badly or stopped trading entirely.
- This is the strongest reason the pooled result was misleading.
- A strategy that is profitable in one regime but refuses to trade or loses in later regimes is not robust enough for live sizing.

### 4. Coverage collapsed on several models

- `rsi_pullback_profit_gate_15m_v1` was basically a refusal-to-trade model.
- `supertrend_adx_profit_gate_15m_smoke` was heavily regime-concentrated and did not generalize cleanly.
- The BB 15m filter also went to zero trades in later folds.
- Low coverage is not a minor issue: a model that only trades in one window can look good in pooled metrics while being unusable forward.

### 5. The feature ceiling is modest

- The 129 normalized technical features produced only weak directional predictiveness on BTC 15m.
- That means more of the same feature family is unlikely to rescue the whole effort.
- The docs point to microstructure / order flow as the next meaningful tier, not more TA indicators.

### 6. The label and cost framing mattered too much

- A `net_r > 0` label downstream of a bad exit model can make a real signal look dead.
- The ledger filter can also bias the sample against recent high-cost consolidation regimes if it removes expensive trades too aggressively.
- These framing issues explain part of the false negative history.
- They also explain why some models improved in paper metrics without becoming economically viable.

## Why We Did Not Break Even

The short version:

1. The signal edge was thin.
2. Fees and slippage were large relative to that edge.
3. Exit logic often removed the good part of the trade.
4. Several models were regime-specific and broke down when the market changed.
5. Some models had good classification scores but still negative expectancy after execution costs.

The longer version:

- BB 5m had strong classification signal, but the 5m timeframe was too expensive relative to the move size.
- BB 15m had a real directional bias, but its profitable pooled result was concentrated in an old regime and vanished later.
- MACD crossover 15m is the first strategy that still survives the target cost regime, but only if execution stays near maker-like conditions.
- At full taker cost, MACD crossover flips negative.
- That means the system is not yet robust enough to tolerate ordinary execution drag without losing the edge.

In practice, the models were often good at learning:

- when a setup was better than random,
- when a regime looked favorable,
- and when to refuse to trade.

But they were not yet strong enough to:

- overcome fees consistently,
- generalize through regime shifts,
- and preserve enough coverage to compound into a stable live result.

## Model-by-Model Summary

- **BB mean reversion 5m**: real classification signal, but too much cost for the small timeframe. Negative expectancy.
- **BB mean reversion 15m**: real raw forward edge, but regime-concentrated and broken by execution assumptions. Dead.
- **MACD crossover 15m**: first conditional deploy candidate. Positive only at low round-trip cost and with horizon-only exits.
- **RSI pullback 15m**: coverage collapsed. Functionally dead.
- **Supertrend ADX smoke 15m**: too concentrated and not robust enough. Dead.
- **Older 5m/10m scans**: useful as historical exploration, but superseded by the current Rust-backed 15m workflow.

## What To Reuse

- Keep the Rust-backed data flow.
- Keep walk-forward evaluation.
- Keep explicit per-trade cost modeling.
- Keep the `fwd_ret_r` style framing when the exit model is the thing destroying signal.
- Keep the hard discipline around deleting one-off scripts after experiments.

## What To Avoid Repeating

- Do not judge a strategy only by pooled metrics.
- Do not trust a `net_r > 0` label if the exit simulation is unrealistic.
- Do not assume a good classifier implies a profitable strategy.
- Do not add throwaway Python scripts for one-off experiments.
- Do not expand the feature set with more TA just because the first batch was weak.

## Practical Conclusion

The work did not fail because the system was broken. It failed to break even because the edge was usually too thin for the cost structure and too fragile across regimes.

The best current path is the one already identified in the run index:

- use the current workflow,
- prefer horizon-preserving exits,
- enforce realistic execution costs,
- and only promote strategies that stay positive across folds and recent years.

