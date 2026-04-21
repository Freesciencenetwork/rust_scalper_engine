//! Best-effort **closed-bar** warmup hints for flattened indicator paths.
//!
//! Counts are in **rows** of the series you send (whatever wall-clock spacing you label in
//! [`crate::machine::MachineRequest::bar_interval`]). The prepare pipeline groups every
//! `higher_tf_factor` consecutive rows into one higher-TF bar for `ema_fast_higher` /
//! `ema_slow_higher` fields.

use crate::config::StrategyConfig;

const NOTE_HIGHER_TF_FIELDS: &str = "Uses the internal higher-TF rollup (every `higher_tf_factor` consecutive rows = one bucket). Warmup count is approximate and depends on your configured factor.";

fn u32_from_cfg(x: usize) -> Option<u32> {
    u32::try_from(x).ok()
}

/// Extra context for paths that need the higher-TF rollup caveat (surfaced in catalog + API docs).
#[must_use]
pub fn path_note(path: &str) -> Option<&'static str> {
    if path.contains("_higher") {
        return Some(NOTE_HIGHER_TF_FIELDS);
    }
    None
}

/// Minimum number of **same-series rows** typically needed before the value at the last bar is
/// defined for this path, given `config`.
///
/// `None` means "not catalogued — infer from `value` / `computable` only".
#[must_use]
pub fn min_bars_required_for_path(path: &str, cfg: &StrategyConfig) -> Option<u32> {
    // --- Top-level [`PreparedCandle`](crate::market_data::PreparedCandle) ---
    match path {
        "candle.open" | "candle.high" | "candle.low" | "candle.close" | "candle.volume"
        | "candle.buy_volume" | "candle.sell_volume" | "candle.delta" | "candle.close_time" => {
            return Some(1);
        }
        "ema_fast" => return u32_from_cfg(cfg.ema_fast_period),
        "ema_slow" => return u32_from_cfg(cfg.ema_slow_period),
        "vwma" => return u32_from_cfg(cfg.vwma_lookback),
        "atr" | "atr_pct" => {
            return u32_from_cfg(cfg.atr_period).map(|n| n.saturating_add(1));
        }
        "atr_pct_baseline" => return u32_from_cfg(cfg.vol_baseline_lookback_bars),
        "vol_ratio" => {
            return Some(
                u32::try_from(
                    (cfg.vol_baseline_lookback_bars)
                        .max(cfg.atr_period + 1)
                        .max(2),
                )
                .unwrap_or(u32::MAX),
            );
        }
        "cvd_ema3" => return Some(8),
        "cvd_ema3_slope" => return Some(12),
        "vp_val" | "vp_poc" | "vp_vah" => {
            if cfg.vp_enabled {
                return u32_from_cfg(cfg.vp_lookback_bars);
            }
            return Some(1);
        }
        "ema_fast_higher" | "ema_slow_higher" => {
            let h = u32::try_from(cfg.ema_slow_period.max(cfg.ema_fast_period)).unwrap_or(u32::MAX);
            let factor = u32::try_from(cfg.higher_tf_factor.max(1)).unwrap_or(1);
            // Rough lower bound in base-row count before the rolled higher-TF series can seed EMAs.
            return Some(h.saturating_mul(factor).saturating_add(factor));
        }
        _ => {}
    }

    // --- [`IndicatorSnapshot`](crate::market_data::snapshot::IndicatorSnapshot) leaves ---
    if let Some(rest) = path.strip_prefix("indicator_snapshot.") {
        return min_bars_for_snapshot_path(rest, cfg);
    }

    None
}

fn min_bars_momentum(rest: &str) -> Option<u32> {
    match rest {
        // `rsi_series` and several fixed-period oscillators: n >= period + 1
        "momentum.rsi_14" | "momentum.williams_r_14" | "momentum.mfi_14" | "momentum.cmo_14" => {
            Some(15)
        }
        "momentum.macd_line"
        | "momentum.macd_signal"
        | "momentum.macd_hist"
        | "momentum.awesome_oscillator_5_34"
        | "momentum.ppo_line"
        | "momentum.ppo_signal"
        | "momentum.ppo_hist" => Some(35),
        "momentum.stoch_k" | "momentum.stoch_d" => Some(20),
        "momentum.stoch_rsi_k" | "momentum.stoch_rsi_d" => Some(40),
        "momentum.cci_20" => Some(21),
        "momentum.roc_10" => Some(11),
        "momentum.ultosc_7_14_28" => Some(29),
        "momentum.tsi_25_13" => Some(26),
        "momentum.kst" => Some(50),
        "momentum.elder_bull" | "momentum.elder_bear" | "momentum.force_index_13" => Some(14),
        "momentum.trix_15" | "momentum.trix_signal_9" => Some(24),
        "momentum.kvo_34_55" | "momentum.kvo_signal_13" => Some(60),
        "momentum.chaikin_oscillator_3_10" => Some(12),
        "momentum.pvo_line" | "momentum.pvo_signal" | "momentum.pvo_hist" => Some(27),
        _ => None,
    }
}

fn min_bars_trend(rest: &str, cfg: &StrategyConfig) -> Option<u32> {
    match rest {
        "trend.sma_20" | "trend.wma_20" | "trend.alma_20" | "trend.ema_20" | "trend.vidya_14" => {
            Some(20)
        }
        "trend.sma_50" | "trend.mama" | "trend.fama" => Some(50),
        "trend.sma_200" => Some(200),
        "trend.hull_9" | "trend.kama_10" => Some(12),
        "trend.dema_20" | "trend.tema_20" => Some(40),
        "trend.mcginley_14" => Some(15),
        "trend.lr_slope_20" | "trend.price_zscore_20" | "trend.hist_vol_logrets_20" => Some(21),
        "trend.vwap_session"
        | "trend.vwap_upper_1sd"
        | "trend.vwap_lower_1sd"
        | "trend.vwap_upper_2sd"
        | "trend.vwap_lower_2sd" => match cfg.vwap_anchor_mode {
            crate::config::VwapAnchorMode::RollingBars => {
                u32_from_cfg(cfg.vwap_rolling_bars.unwrap_or(96))
            }
            crate::config::VwapAnchorMode::UtcDay => Some(96),
            crate::config::VwapAnchorMode::Disabled => Some(1),
        },
        _ => None,
    }
}

fn min_bars_volatility(rest: &str) -> Option<u32> {
    if rest.starts_with("volatility.pivot_classic.") || rest.starts_with("volatility.pivot_fib.") {
        return Some(2);
    }
    match rest {
        "volatility.bb_middle_20"
        | "volatility.bb_upper_20"
        | "volatility.bb_lower_20"
        | "volatility.bb_pct_b_20"
        | "volatility.bb_bandwidth_20"
        | "volatility.donchian_upper_20"
        | "volatility.donchian_lower_20"
        | "volatility.donchian_mid_20"
        | "volatility.ttm_squeeze_on"
        | "volatility.ttm_squeeze_momentum" => Some(20),
        "volatility.keltner_middle_20"
        | "volatility.keltner_upper_20"
        | "volatility.keltner_lower_20" => Some(21),
        "volatility.supertrend_10_3" | "volatility.supertrend_long" => Some(12),
        "volatility.mass_index_25" => Some(26),
        "volatility.chandelier_long_stop_22_3" | "volatility.chandelier_short_stop_22_3" => {
            Some(23)
        }
        _ => None,
    }
}

fn min_bars_directional(rest: &str) -> Option<u32> {
    match rest {
        "directional.adx_14"
        | "directional.di_plus"
        | "directional.di_minus"
        | "directional.vortex_vi_plus_14"
        | "directional.vortex_vi_minus_14" => Some(15),
        "directional.aroon_up_25" | "directional.aroon_down_25" => Some(25),
        "directional.psar" | "directional.psar_trend_long" => Some(10),
        _ => None,
    }
}

fn min_bars_volume_paths(rest: &str) -> Option<u32> {
    match rest {
        "volume.obv" | "volume.ad_line" | "volume.nvi" | "volume.pvi" => Some(2),
        "volume.cmf_20" => Some(21),
        "volume.volume_sma_20" | "volume.volume_ema_20" => Some(20),
        _ => None,
    }
}

fn min_bars_for_snapshot_path(rest: &str, cfg: &StrategyConfig) -> Option<u32> {
    if rest.starts_with("momentum.") {
        return min_bars_momentum(rest);
    }

    if rest.starts_with("trend.") {
        return min_bars_trend(rest, cfg);
    }

    if rest.starts_with("ichimoku.") {
        return Some(52);
    }

    if rest.starts_with("volatility.") {
        return min_bars_volatility(rest);
    }

    if rest.starts_with("directional.") {
        return min_bars_directional(rest);
    }

    if rest.starts_with("volume.") {
        return min_bars_volume_paths(rest);
    }

    if rest.starts_with("patterns.") {
        return Some(3);
    }

    None
}
