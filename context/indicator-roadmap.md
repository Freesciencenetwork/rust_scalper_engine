# Indicator roadmap — add all missing TV-common studies

Scope: **library indicators only** (one file per study under `src/indicators/`), wired into `IndicatorSnapshot` via `market_data/prepare.rs`. **Default strategy gates unchanged** unless explicitly decided later.

## Already implemented (do not duplicate)

- EMA, SMA, WMA, ATR, VWMA (rolling), RSI, MACD, BB (mid/upper/lower), Stochastic, CCI, Williams %R, OBV, ADX/DI±, MFI, ROC, Keltner, Donchian, Aroon, Hull MA, Ultimate Oscillator, TSI, rolling **volume profile** (POC/VAL/VAH on `PreparedCandle`, not only snapshot).

---

## Tier 1 — Volume / money flow (OHLCV only)

| # | Study | File | Outputs | `IndicatorSnapshot` |
|---|--------|------|---------|----------------------|
| 1 | Accumulation/Distribution (A/D line) | `ad_line.rs` | `Vec<f64>` cumulative | `volume.ad_line` |
| 2 | Chaikin Money Flow | `cmf.rs` | `Vec<Option<f64>>` | `volume.cmf_20` |
| 3 | Volume SMA | `volume_sma.rs` | `Vec<Option<f64>>` | `volume.volume_sma_20` |
| 4 | Volume EMA | `volume_ema.rs` | `Vec<f64>` (optional) | `volume.volume_ema_20` |

**Notes:** CMF period default 20. A/D is cumulative; store level + optional slope if needed later.

---

## Tier 2 — Bollinger derivatives

| # | Study | File | Outputs | Snapshot |
|---|--------|------|---------|----------|
| 5 | Bollinger %B | `bollinger_pct_b.rs` | `Vec<Option<f64>>` | `volatility.bb_pct_b_20` |
| 6 | Bollinger Bandwidth | `bollinger_bandwidth.rs` | `Vec<Option<f64>>` | `volatility.bb_bandwidth_20` |

**Notes:** Derive from existing `bollinger_series` or shared helper to avoid double SMA cost.

---

## Tier 3 — Momentum / oscillators

| # | Study | File | Outputs | Snapshot |
|---|--------|------|---------|----------|
| 7 | Stochastic RSI | `stoch_rsi.rs` | K, D `Vec<Option<f64>>` | `momentum.stoch_rsi_k`, `momentum.stoch_rsi_d` |
| 8 | Awesome Oscillator | `awesome_oscillator.rs` | `Vec<f64>` | `momentum.awesome_oscillator_5_34` |
| 9 | PPO (Percentage Price Oscillator) | `ppo.rs` | line, signal, hist | `momentum.ppo_*` (optional; overlaps MACD scaling) |

**Notes:** Stoch RSI: RSI length → apply stochastic formula. AO: median price = (H+L)/2, SMA5 − SMA34 of median (TV defaults).

---

## Tier 4 — Trend overlays (stateful)

| # | Study | File | Outputs | Snapshot |
|---|--------|------|---------|----------|
| 10 | Parabolic SAR | `parabolic_sar.rs` | sar, `is_long` per bar | `directional.psar`, `directional.psar_trend_long` (bool as 0/1) |
| 11 | SuperTrend | `supertrend.rs` | line, direction | `volatility.supertrend_10_3`, `volatility.supertrend_long` |

**Notes:** Single forward pass from bar 0; AF/step defaults (SAR 0.02, 0.2; ST ATR mult 3, length 10). Document TV parity vs simplified rules.

---

## Tier 5 — VWAP family (config + anchor)

| # | Study | File | Outputs | Snapshot |
|---|--------|------|---------|----------|
| 12 | Session VWAP | `vwap.rs` | VWAP, optional ±1σ, ±2σ | `trend.vwap_session`, `trend.vwap_upper_1sd`, … |

**Config (add to `StrategyConfig` or nested `VwapConfig`):**

- `vwap_anchor: enum { UtcDay, RollingBars(usize), None }` — recommend **UtcDay** first for crypto 15m.
- `vwap_include_current_bar: bool` — TV often includes forming candle; decide and document.

**Reset rule:** On each new anchor window, cum `typical_price * volume` / cum `volume` from window start.

---

## Tier 6 — Ichimoku Cloud

| # | Study | File | Outputs | Snapshot |
|---|--------|------|---------|----------|
| 13 | Ichimoku | `ichimoku.rs` | tenkan, kijun, senkou_a, senkou_b, chikou (shifted per spec) | New struct `IchimokuSnapshot` nested in `IndicatorSnapshot` |

**Fields:** `tenkan_9`, `kijun_26`, `senkou_a_26`, `senkou_b_52`, `chikou_close_shifted` — align displacements (+26, -26) exactly as TV or document offset.

---

## Tier 7 — Pivots & session structure

| # | Study | File | Outputs | Snapshot |
|---|--------|------|---------|----------|
| 14 | Classic pivots (daily) | `pivot_classic.rs` | P, R1–R3, S1–S3 | `volatility.pivot_p`, `pivot_r1`, … |
| 15 | Fib pivots (optional) | `pivot_fib.rs` | same pattern | optional second block |

**Notes:** Needs **prior session** high/low/close; use UTC day boundary on `close_time` for consistency with VWAP anchor.

---

## Tier 8 — Moving average variants (low priority)

| # | Study | File | Snapshot |
|---|--------|------|----------|
| 16 | DEMA | `dema.rs` | `trend.dema_20` |
| 17 | TEMA | `tema.rs` | `trend.tema_20` |
| 18 | McGinley Dynamic | `mcginley.rs` | `trend.mcginley_14` |

**Notes:** Implemented via EMA-of-EMA formulas; few lines each.

---

## Tier 9 — Optional / niche

| # | Study | File | Snapshot |
|---|--------|------|----------|
| 19 | Know Sure Thing (KST) | `kst.rs` | `momentum.kst` |
| 20 | Elder Ray | `elder_ray.rs` | bull/bear power | `momentum.elder_bull`, `momentum.elder_bear` |
| 21 | Mass Index | `mass_index.rs` | `volatility.mass_index_25` |

---

## Implementation checklist (repeat per study)

1. Add `src/indicators/<file>.rs` with `series(...)` API + `#[cfg(test)]` for non-trivial logic.
2. Export in `src/indicators/mod.rs`.
3. Extend `src/market_data/snapshot.rs` (new nested struct if ≥4 related fields).
4. Compute once in `PreparedDataset::build` in `prepare.rs`; map index → snapshot fields.
5. Run `cargo test`; add fixture note if JSON size grows.
6. Update `context/state.md` one-line changelog.

---

## Recommended implementation order

1. **Tier 1** (A/D, CMF, volume SMA/EMA) — no state, high TV usage.  
2. **Tier 2** (%B, bandwidth).  
3. **Tier 3** (Stoch RSI, AO; skip PPO unless you want MACD % duplicate).  
4. **Tier 4** (PSAR, SuperTrend) — implement stateful pass + tests.  
5. **Tier 5** (VWAP) — **after** `VwapAnchor` config merged.  
6. **Tier 6** (Ichimoku).  
7. **Tier 7** (Pivots) — shares session boundary with VWAP.  
8. **Tier 8–9** as needed.

---

## Risks / scope control

- **Serde / API payload size:** prefer nested structs (`IchimokuSnapshot`, `VwapSnapshot`) and `#[serde(skip_serializing_if = "Option::is_none")]` if clients complain.  
- **TV exact parity:** document defaults; add golden-vector tests only for critical studies (VWAP, Ichimoku, SAR).  
- **Performance:** precompute each `Vec` once per dataset build (current pattern); avoid per-bar realloc.

---

## Definition of done

- Every row in tiers 1–7 has a corresponding `indicators/*.rs`, snapshot fields, and `prepare.rs` wiring.  
- Tiers 8–9 optional flag in this doc when implemented.  
- `cargo test` green; default strategy behavior unchanged.
