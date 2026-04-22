"""
normalize_features.py — Convert raw indicator values into scale-invariant features.

Problem with raw indicators
---------------------------
Many indicators are in price or volume units. Feeding them raw to a model
trained on 2012-2026 data would teach it "BTC was $100 in 2012" — pure
regime memorization that would not generalise.

Normalization rules
-------------------
GROUP 1  Price-level → (value - close) / close
  Tells the model: "how far is this level from current price?"
  Applies to: all moving averages, VWAP bands, Bollinger bands,
  Keltner channels, Donchian channels, Ichimoku cloud lines,
  pivot levels, chandelier stops, PSAR, supertrend, volume profile levels.

GROUP 2  MACD-type (dollar momentum) → value / close
  Preserves sign, removes price-scale dependence.
  Applies to: MACD line/signal/hist, awesome oscillator,
  elder bull/bear, KVO, force index, linear regression slope,
  TTM squeeze momentum.

GROUP 3  Cumulative volume → rolling z-score (window=120, no lookahead)
  OBV / AD line / NVI / PVI grow indefinitely; the z-score captures
  recent deviation from local mean without regime drift.

GROUP 4  Already dimensionless → keep as-is
  RSI, Stochastic, Williams %R, ADX, CCI, MFI, PPO, CMF, TSI, etc.
  These are already bounded or ratio-scaled.

GROUP 5  Binary / categorical → keep as-is
  Session flags, candlestick patterns, trend direction bits.

GROUP 6  Redundant / raw → DROP
  candle.close (IS the denominator), raw atr (covered by atr_pct).

Output
------
data/features_normalized.parquet
  Columns: timestamp_ms  +  <feature columns (underscore names)>
  No raw price-unit values. NaN for warm-up rows.
"""

import json
import logging
import os
import sys

import numpy as np
import pandas as pd

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)

IN_PATH  = "data/indicators_full.parquet"
OUT_PATH = "data/features_normalized.parquet"
IN_METADATA_PATH = "data/indicators_full.metadata.json"
OUT_METADATA_PATH = "data/features_normalized.metadata.json"

# ── Column name helper ────────────────────────────────────────────────────────

def _name(raw: str) -> str:
    """Strip 'indicator_snapshot.' prefix and replace dots with underscores."""
    return raw.replace("indicator_snapshot.", "").replace(".", "_")


# ── Group 1: price-level indicators (normalize as (value - close) / close) ───

PRICE_LEVEL = [
    "ema_fast", "ema_slow", "vwma",
    "vp_poc", "vp_vah", "vp_val",
    "indicator_snapshot.ichimoku.chikou_close_shifted",
    "indicator_snapshot.ichimoku.kijun_26",
    "indicator_snapshot.ichimoku.senkou_a_26",
    "indicator_snapshot.ichimoku.senkou_b_52",
    "indicator_snapshot.ichimoku.tenkan_9",
    "indicator_snapshot.trend.alma_20",
    "indicator_snapshot.trend.dema_20",
    "indicator_snapshot.trend.ema_20",
    "indicator_snapshot.trend.fama",
    "indicator_snapshot.trend.hull_9",
    "indicator_snapshot.trend.kama_10",
    "indicator_snapshot.trend.mama",
    "indicator_snapshot.trend.mcginley_14",
    "indicator_snapshot.trend.sma_20",
    "indicator_snapshot.trend.sma_200",
    "indicator_snapshot.trend.sma_50",
    "indicator_snapshot.trend.tema_20",
    "indicator_snapshot.trend.vidya_14",
    "indicator_snapshot.trend.vwap_lower_1sd",
    "indicator_snapshot.trend.vwap_lower_2sd",
    "indicator_snapshot.trend.vwap_session",
    "indicator_snapshot.trend.vwap_upper_1sd",
    "indicator_snapshot.trend.vwap_upper_2sd",
    "indicator_snapshot.trend.wma_20",
    "indicator_snapshot.volatility.bb_lower_20",
    "indicator_snapshot.volatility.bb_middle_20",
    "indicator_snapshot.volatility.bb_upper_20",
    "indicator_snapshot.volatility.chandelier_long_stop_22_3",
    "indicator_snapshot.volatility.chandelier_short_stop_22_3",
    "indicator_snapshot.volatility.donchian_lower_20",
    "indicator_snapshot.volatility.donchian_mid_20",
    "indicator_snapshot.volatility.donchian_upper_20",
    "indicator_snapshot.volatility.keltner_lower_20",
    "indicator_snapshot.volatility.keltner_middle_20",
    "indicator_snapshot.volatility.keltner_upper_20",
    "indicator_snapshot.volatility.pivot_classic.pivot_p",
    "indicator_snapshot.volatility.pivot_classic.pivot_r1",
    "indicator_snapshot.volatility.pivot_classic.pivot_r2",
    "indicator_snapshot.volatility.pivot_classic.pivot_r3",
    "indicator_snapshot.volatility.pivot_classic.pivot_s1",
    "indicator_snapshot.volatility.pivot_classic.pivot_s2",
    "indicator_snapshot.volatility.pivot_classic.pivot_s3",
    "indicator_snapshot.volatility.pivot_fib.pivot_p",
    "indicator_snapshot.volatility.pivot_fib.pivot_r1",
    "indicator_snapshot.volatility.pivot_fib.pivot_r2",
    "indicator_snapshot.volatility.pivot_fib.pivot_r3",
    "indicator_snapshot.volatility.pivot_fib.pivot_s1",
    "indicator_snapshot.volatility.pivot_fib.pivot_s2",
    "indicator_snapshot.volatility.pivot_fib.pivot_s3",
    "indicator_snapshot.volatility.supertrend_10_3",
    "indicator_snapshot.directional.psar",
]

# ── Group 2: dollar-momentum (normalize as value / close) ────────────────────

DOLLAR_MOMENTUM = [
    "indicator_snapshot.momentum.awesome_oscillator_5_34",
    "indicator_snapshot.momentum.elder_bear",
    "indicator_snapshot.momentum.elder_bull",
    "indicator_snapshot.momentum.force_index_13",
    "indicator_snapshot.momentum.kvo_34_55",
    "indicator_snapshot.momentum.kvo_signal_13",
    "indicator_snapshot.momentum.macd_hist",
    "indicator_snapshot.momentum.macd_line",
    "indicator_snapshot.momentum.macd_signal",
    "indicator_snapshot.trend.lr_slope_20",
    "indicator_snapshot.volatility.ttm_squeeze_momentum",
]

# ── Group 3: cumulative volume → rolling z-score ──────────────────────────────

CUMULATIVE_VOL = [
    "indicator_snapshot.volume.obv",
    "indicator_snapshot.volume.ad_line",
    "indicator_snapshot.volume.nvi",
    "indicator_snapshot.volume.pvi",
    "indicator_snapshot.volume.volume_ema_20",
    "indicator_snapshot.volume.volume_sma_20",
]
VOL_ZSCORE_WIN = 120   # 2 hours on 1m bars; long enough to capture local trend

# ── Group 4: already dimensionless → keep as-is ──────────────────────────────

DIMENSIONLESS = [
    "atr_pct", "atr_pct_baseline", "vol_ratio",
    "cvd_ema3", "cvd_ema3_slope",
    "indicator_snapshot.directional.adx_14",
    "indicator_snapshot.directional.aroon_down_25",
    "indicator_snapshot.directional.aroon_up_25",
    "indicator_snapshot.directional.di_minus",
    "indicator_snapshot.directional.di_plus",
    "indicator_snapshot.directional.vortex_vi_minus_14",
    "indicator_snapshot.directional.vortex_vi_plus_14",
    "indicator_snapshot.momentum.cci_20",
    "indicator_snapshot.momentum.chaikin_oscillator_3_10",
    "indicator_snapshot.momentum.cmo_14",
    "indicator_snapshot.momentum.kst",
    "indicator_snapshot.momentum.mfi_14",
    "indicator_snapshot.momentum.ppo_hist",
    "indicator_snapshot.momentum.ppo_line",
    "indicator_snapshot.momentum.ppo_signal",
    "indicator_snapshot.momentum.pvo_hist",
    "indicator_snapshot.momentum.pvo_line",
    "indicator_snapshot.momentum.pvo_signal",
    "indicator_snapshot.momentum.roc_10",
    "indicator_snapshot.momentum.rsi_14",
    "indicator_snapshot.momentum.stoch_d",
    "indicator_snapshot.momentum.stoch_k",
    "indicator_snapshot.momentum.stoch_rsi_d",
    "indicator_snapshot.momentum.stoch_rsi_k",
    "indicator_snapshot.momentum.trix_15",
    "indicator_snapshot.momentum.trix_signal_9",
    "indicator_snapshot.momentum.tsi_25_13",
    "indicator_snapshot.momentum.ultosc_7_14_28",
    "indicator_snapshot.momentum.williams_r_14",
    "indicator_snapshot.order_flow.vwap_deviation_pct",
    "indicator_snapshot.trend.hist_vol_logrets_20",
    "indicator_snapshot.trend.price_zscore_20",
    "indicator_snapshot.volatility.bb_bandwidth_20",
    "indicator_snapshot.volatility.bb_pct_b_20",
    "indicator_snapshot.volatility.mass_index_25",
    "indicator_snapshot.volume.cmf_20",
]

# ── Group 5: binary / categorical → keep as-is ───────────────────────────────

BINARY = [
    "indicator_snapshot.directional.psar_trend_long",
    "indicator_snapshot.order_flow.in_asia_session",
    "indicator_snapshot.order_flow.in_eu_session",
    "indicator_snapshot.order_flow.in_us_session",
    "indicator_snapshot.order_flow.liquidity_sweep_down",
    "indicator_snapshot.order_flow.liquidity_sweep_up",
    "indicator_snapshot.order_flow.thin_zone",
    "indicator_snapshot.patterns.bear_engulfing",
    "indicator_snapshot.patterns.bull_engulfing",
    "indicator_snapshot.patterns.doji",
    "indicator_snapshot.patterns.hammer",
    "indicator_snapshot.patterns.shooting_star",
    "indicator_snapshot.volatility.supertrend_long",
    "indicator_snapshot.volatility.ttm_squeeze_on",
]

# ── Group 6: DROP ─────────────────────────────────────────────────────────────
# candle.close  — is the denominator, not a feature
# atr           — covered by atr_pct (already ratio)
# candle.volume — raw, covered by volume z-score in Groups 3+4


def normalize(df: pd.DataFrame) -> pd.DataFrame:
    """
    Apply all normalization rules and return a clean feature DataFrame.
    Input must contain 'candle.close' column and all indicator columns.
    """
    close = df["candle.close"].replace(0, np.nan)   # guard against zero close
    out_frames = [df[["timestamp_ms"]].copy()]

    # ── Group 1: price-level → (value - close) / close ──────────────────────
    price_level = {}
    for col in PRICE_LEVEL:
        if col not in df.columns:
            continue
        price_level[_name(col) + "_rel"] = (df[col] - close) / close
    if price_level:
        out_frames.append(pd.DataFrame(price_level, index=df.index))

    # ── Group 2: dollar-momentum → value / close ─────────────────────────────
    dollar_momentum = {}
    for col in DOLLAR_MOMENTUM:
        if col not in df.columns:
            continue
        dollar_momentum[_name(col) + "_norm"] = df[col] / close
    if dollar_momentum:
        out_frames.append(pd.DataFrame(dollar_momentum, index=df.index))

    # ── Group 3: cumulative volume → rolling z-score ──────────────────────────
    # shift(1) so bar t's own value is not used to compute its own z-score
    cumulative_vol = {}
    for col in CUMULATIVE_VOL:
        if col not in df.columns:
            continue
        series = df[col]
        roll   = series.shift(1).rolling(VOL_ZSCORE_WIN, min_periods=VOL_ZSCORE_WIN // 2)
        mu     = roll.mean()
        sigma  = roll.std().replace(0, np.nan)
        cumulative_vol[_name(col) + "_zscore"] = (series - mu) / sigma
    if cumulative_vol:
        out_frames.append(pd.DataFrame(cumulative_vol, index=df.index))

    # ── Group 4: dimensionless → keep with clean name ─────────────────────────
    dimensionless = {}
    for col in DIMENSIONLESS:
        if col not in df.columns:
            continue
        dimensionless[_name(col)] = df[col]
    if dimensionless:
        out_frames.append(pd.DataFrame(dimensionless, index=df.index))

    # ── Group 5: binary → keep with clean name ────────────────────────────────
    binary = {}
    for col in BINARY:
        if col not in df.columns:
            continue
        binary[_name(col)] = df[col]
    if binary:
        out_frames.append(pd.DataFrame(binary, index=df.index))

    out = pd.concat(out_frames, axis=1)
    out.replace([np.inf, -np.inf], np.nan, inplace=True)
    return out


# ── Published feature column list (everything except timestamp_ms) ───────────

def feature_columns(df: pd.DataFrame) -> list:
    return [c for c in df.columns if c != "timestamp_ms"]


def main():
    logger.info("Loading %s ...", IN_PATH)
    raw = pd.read_parquet(IN_PATH)
    logger.info("Raw shape: %s", raw.shape)

    if not os.path.exists(IN_METADATA_PATH):
        raise FileNotFoundError(
            f"Missing Rust feature metadata: {IN_METADATA_PATH}. Run fetch_indicators.py first."
        )
    with open(IN_METADATA_PATH) as fh:
        raw_metadata = json.load(fh)
    if raw_metadata.get("source") != "rust_backend":
        raise ValueError(f"{IN_METADATA_PATH} is not marked as rust_backend source.")

    logger.info("Normalizing ...")
    norm = normalize(raw)

    n_features = len(feature_columns(norm))
    logger.info("Normalized shape: %s  (%d features)", norm.shape, n_features)

    nan_pct = norm.isnull().mean()
    high_nan = nan_pct[nan_pct > 0.10].sort_values(ascending=False)
    if not high_nan.empty:
        logger.warning("Features with >10%% NaN (warm-up or data gap):")
        for col, pct in high_nan.items():
            logger.warning("  %s: %.1f%%", col, 100 * pct)

    os.makedirs("data", exist_ok=True)
    norm.to_parquet(OUT_PATH, index=False, compression="snappy")
    logger.info("Saved -> %s  (%.1f MB)",
                OUT_PATH, os.path.getsize(OUT_PATH) / 1e6)

    metadata = {
        "source": "rust_backend",
        "pipeline_stage": "normalized_features",
        "derived_from": IN_PATH,
        "derived_from_metadata": IN_METADATA_PATH,
        "rows": int(len(norm)),
        "columns": list(norm.columns),
        "feature_count": n_features,
    }
    with open(OUT_METADATA_PATH, "w") as fh:
        json.dump(metadata, fh, indent=2)
    logger.info("Saved metadata -> %s", OUT_METADATA_PATH)

    # Print feature list for reference
    logger.info("Feature columns (%d):", n_features)
    for col in sorted(feature_columns(norm)):
        logger.info("  %s", col)


if __name__ == "__main__":
    main()
