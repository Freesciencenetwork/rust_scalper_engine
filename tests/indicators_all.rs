//! Exercises every indicator wired in `PreparedDataset::build` (see `market_data/prepare.rs`).
//! Uses synthetic candles; `higher_tf_factor` defaults to 4 so groups of 4 bars form one higher-TF bar.

#![allow(clippy::pedantic, clippy::nursery)] // Large synthetic candle harness; pedantic on test loops is low value.

use binance_BTC::{Candle, PreparedDataset, StrategyConfig, VwapAnchorMode};
use chrono::{Duration, TimeZone, Utc};

const BAR_COUNT: usize = 260;

fn assert_finite_opt(name: &str, value: Option<f64>) {
    if let Some(v) = value {
        assert!(v.is_finite(), "{name} expected finite f64, got {v:?}");
    }
}

fn assert_momentum_finite(m: &binance_BTC::MomentumSnapshot) {
    assert_finite_opt("rsi_14", m.rsi_14);
    assert_finite_opt("macd_line", m.macd_line);
    assert_finite_opt("macd_signal", m.macd_signal);
    assert_finite_opt("macd_hist", m.macd_hist);
    assert_finite_opt("stoch_k", m.stoch_k);
    assert_finite_opt("stoch_d", m.stoch_d);
    assert_finite_opt("stoch_rsi_k", m.stoch_rsi_k);
    assert_finite_opt("stoch_rsi_d", m.stoch_rsi_d);
    assert_finite_opt("cci_20", m.cci_20);
    assert_finite_opt("williams_r_14", m.williams_r_14);
    assert_finite_opt("roc_10", m.roc_10);
    assert_finite_opt("mfi_14", m.mfi_14);
    assert_finite_opt("ultosc_7_14_28", m.ultosc_7_14_28);
    assert_finite_opt("tsi_25_13", m.tsi_25_13);
    assert_finite_opt("awesome_oscillator_5_34", m.awesome_oscillator_5_34);
    assert_finite_opt("ppo_line", m.ppo_line);
    assert_finite_opt("ppo_signal", m.ppo_signal);
    assert_finite_opt("ppo_hist", m.ppo_hist);
    assert_finite_opt("kst", m.kst);
    assert_finite_opt("elder_bull", m.elder_bull);
    assert_finite_opt("elder_bear", m.elder_bear);
    assert_finite_opt("cmo_14", m.cmo_14);
    assert_finite_opt("trix_15", m.trix_15);
    assert_finite_opt("trix_signal_9", m.trix_signal_9);
    assert_finite_opt("kvo_34_55", m.kvo_34_55);
    assert_finite_opt("kvo_signal_13", m.kvo_signal_13);
    assert_finite_opt("chaikin_oscillator_3_10", m.chaikin_oscillator_3_10);
    assert_finite_opt("pvo_line", m.pvo_line);
    assert_finite_opt("pvo_signal", m.pvo_signal);
    assert_finite_opt("pvo_hist", m.pvo_hist);
    assert_finite_opt("force_index_13", m.force_index_13);
}

fn assert_trend_finite(t: &binance_BTC::TrendSnapshot) {
    assert_finite_opt("sma_20", t.sma_20);
    assert_finite_opt("sma_50", t.sma_50);
    assert_finite_opt("sma_200", t.sma_200);
    assert_finite_opt("ema_20", t.ema_20);
    assert_finite_opt("wma_20", t.wma_20);
    assert_finite_opt("hull_9", t.hull_9);
    assert_finite_opt("vwap_session", t.vwap_session);
    assert_finite_opt("vwap_upper_1sd", t.vwap_upper_1sd);
    assert_finite_opt("vwap_lower_1sd", t.vwap_lower_1sd);
    assert_finite_opt("vwap_upper_2sd", t.vwap_upper_2sd);
    assert_finite_opt("vwap_lower_2sd", t.vwap_lower_2sd);
    assert_finite_opt("dema_20", t.dema_20);
    assert_finite_opt("tema_20", t.tema_20);
    assert_finite_opt("mcginley_14", t.mcginley_14);
    assert_finite_opt("kama_10", t.kama_10);
    assert_finite_opt("alma_20", t.alma_20);
    assert_finite_opt("vidya_14", t.vidya_14);
    assert_finite_opt("mama", t.mama);
    assert_finite_opt("fama", t.fama);
    assert_finite_opt("lr_slope_20", t.lr_slope_20);
    assert_finite_opt("price_zscore_20", t.price_zscore_20);
    assert_finite_opt("hist_vol_logrets_20", t.hist_vol_logrets_20);
}

fn assert_ichimoku_finite(i: &binance_BTC::IchimokuSnapshot) {
    assert_finite_opt("tenkan_9", i.tenkan_9);
    assert_finite_opt("kijun_26", i.kijun_26);
    assert_finite_opt("senkou_a_26", i.senkou_a_26);
    assert_finite_opt("senkou_b_52", i.senkou_b_52);
    assert_finite_opt("chikou_close_shifted", i.chikou_close_shifted);
}

fn assert_pivot_classic_finite(p: &binance_BTC::PivotClassicSnapshot) {
    assert_finite_opt("pivot_p", p.pivot_p);
    assert_finite_opt("pivot_r1", p.pivot_r1);
    assert_finite_opt("pivot_r2", p.pivot_r2);
    assert_finite_opt("pivot_r3", p.pivot_r3);
    assert_finite_opt("pivot_s1", p.pivot_s1);
    assert_finite_opt("pivot_s2", p.pivot_s2);
    assert_finite_opt("pivot_s3", p.pivot_s3);
}

fn assert_pivot_fib_finite(p: &binance_BTC::PivotFibSnapshot) {
    assert_finite_opt("fib_pivot_p", p.pivot_p);
    assert_finite_opt("fib_pivot_r1", p.pivot_r1);
    assert_finite_opt("fib_pivot_r2", p.pivot_r2);
    assert_finite_opt("fib_pivot_r3", p.pivot_r3);
    assert_finite_opt("fib_pivot_s1", p.pivot_s1);
    assert_finite_opt("fib_pivot_s2", p.pivot_s2);
    assert_finite_opt("fib_pivot_s3", p.pivot_s3);
}

fn assert_volatility_finite(v: &binance_BTC::VolatilitySnapshot) {
    assert_finite_opt("bb_middle_20", v.bb_middle_20);
    assert_finite_opt("bb_upper_20", v.bb_upper_20);
    assert_finite_opt("bb_lower_20", v.bb_lower_20);
    assert_finite_opt("bb_pct_b_20", v.bb_pct_b_20);
    assert_finite_opt("bb_bandwidth_20", v.bb_bandwidth_20);
    assert_finite_opt("keltner_middle_20", v.keltner_middle_20);
    assert_finite_opt("keltner_upper_20", v.keltner_upper_20);
    assert_finite_opt("keltner_lower_20", v.keltner_lower_20);
    assert_finite_opt("donchian_upper_20", v.donchian_upper_20);
    assert_finite_opt("donchian_lower_20", v.donchian_lower_20);
    assert_finite_opt("donchian_mid_20", v.donchian_mid_20);
    assert_finite_opt("supertrend_10_3", v.supertrend_10_3);
    assert_finite_opt("mass_index_25", v.mass_index_25);
    assert_finite_opt("ttm_squeeze_momentum", v.ttm_squeeze_momentum);
    assert_finite_opt("chandelier_long_stop_22_3", v.chandelier_long_stop_22_3);
    assert_finite_opt("chandelier_short_stop_22_3", v.chandelier_short_stop_22_3);
    assert_pivot_classic_finite(&v.pivot_classic);
    assert_pivot_fib_finite(&v.pivot_fib);
}

fn assert_directional_finite(d: &binance_BTC::DirectionalSnapshot) {
    assert_finite_opt("adx_14", d.adx_14);
    assert_finite_opt("di_plus", d.di_plus);
    assert_finite_opt("di_minus", d.di_minus);
    assert_finite_opt("aroon_up_25", d.aroon_up_25);
    assert_finite_opt("aroon_down_25", d.aroon_down_25);
    assert_finite_opt("psar", d.psar);
    assert_finite_opt("vortex_vi_plus_14", d.vortex_vi_plus_14);
    assert_finite_opt("vortex_vi_minus_14", d.vortex_vi_minus_14);
}

fn assert_volume_finite(vol: &binance_BTC::VolumeSnapshot) {
    assert_finite_opt("obv", vol.obv);
    assert_finite_opt("ad_line", vol.ad_line);
    assert_finite_opt("cmf_20", vol.cmf_20);
    assert_finite_opt("volume_sma_20", vol.volume_sma_20);
    assert_finite_opt("volume_ema_20", vol.volume_ema_20);
    assert_finite_opt("nvi", vol.nvi);
    assert_finite_opt("pvi", vol.pvi);
}

fn assert_indicator_snapshot_finite(s: &binance_BTC::IndicatorSnapshot) {
    assert_momentum_finite(&s.momentum);
    assert_trend_finite(&s.trend);
    assert_ichimoku_finite(&s.ichimoku);
    assert_volatility_finite(&s.volatility);
    assert_directional_finite(&s.directional);
    assert_volume_finite(&s.volume);
}

fn assert_prepared_fields_finite(frame: &binance_BTC::PreparedCandle) {
    assert_finite_opt("ema_fast", frame.ema_fast);
    assert_finite_opt("ema_slow", frame.ema_slow);
    assert_finite_opt("ema_fast_higher", frame.ema_fast_higher);
    assert_finite_opt("ema_slow_higher", frame.ema_slow_higher);
    assert_finite_opt("vwma", frame.vwma);
    assert_finite_opt("atr", frame.atr);
    assert_finite_opt("atr_pct", frame.atr_pct);
    assert_finite_opt("atr_pct_baseline", frame.atr_pct_baseline);
    assert_finite_opt("vol_ratio", frame.vol_ratio);
    assert_finite_opt("cvd_ema3", frame.cvd_ema3);
    assert_finite_opt("cvd_ema3_slope", frame.cvd_ema3_slope);
    assert_finite_opt("vp_val", frame.vp_val);
    assert_finite_opt("vp_poc", frame.vp_poc);
    assert_finite_opt("vp_vah", frame.vp_vah);
}

/// Synthetic bars stepping 15 minutes from 00:15 UTC (any timeframe label; engine is bar-count based).
fn synthetic_candles_15m(count: usize) -> Vec<Candle> {
    assert!(
        count >= 4,
        "need at least 4 bars to form one higher-TF bar at default factor"
    );
    let base = Utc.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap();
    let mut out = Vec::with_capacity(count);
    let mut price = 100.0_f64;
    for i in 0..count {
        let minute = 15_i64 + 15_i64 * i as i64;
        // Small drift so oscillators see movement.
        price += (i as f64 * 0.017).sin() * 0.05;
        let o = price - 0.02;
        let c = price + 0.01;
        let h = price + 0.5;
        let l = price - 0.5;
        let vol = 12.0 + (i % 7) as f64;
        out.push(Candle {
            close_time: base + Duration::minutes(minute),
            open: o,
            high: h,
            low: l,
            close: c,
            volume: vol,
            buy_volume: Some(vol * 0.55),
            sell_volume: Some(vol * 0.45),
            delta: None,
        });
    }
    out
}

fn config_for_full_indicator_warmup() -> StrategyConfig {
    StrategyConfig {
        // Keep rolling windows ≤ BAR_COUNT so medians / VWMA / VP windows fill.
        vol_baseline_lookback_bars: 120,
        vwma_lookback: 96,
        vp_enabled: true,
        vp_lookback_bars: 96,
        vp_bin_count: 24,
        vp_value_area_ratio: 0.7,
        vwap_anchor_mode: VwapAnchorMode::RollingBars,
        vwap_rolling_bars: Some(96),
        vwap_include_current_bar: true,
        ..Default::default()
    }
}

#[test]
fn prepared_dataset_computes_all_indicators_without_panic() {
    let config = config_for_full_indicator_warmup();
    let candles = synthetic_candles_15m(BAR_COUNT);
    let dataset = PreparedDataset::build(&config, candles, vec![]).expect("prepared dataset");
    assert_eq!(dataset.frames.len(), BAR_COUNT);
}

#[test]
fn last_bar_indicator_values_are_finite() {
    let config = config_for_full_indicator_warmup();
    let candles = synthetic_candles_15m(BAR_COUNT);
    let dataset = PreparedDataset::build(&config, candles, vec![]).expect("prepared dataset");
    let last = dataset.frames.last().expect("at least one frame");

    assert_prepared_fields_finite(last);
    assert_indicator_snapshot_finite(&last.indicator_snapshot);

    // Warmup spot-checks (longest SMA + core oscillators).
    let s = &last.indicator_snapshot;
    assert!(
        s.trend.sma_200.is_some(),
        "sma_200 should be defined after 260 bars"
    );
    assert!(s.momentum.rsi_14.is_some(), "rsi_14 should be defined");
    assert!(s.directional.adx_14.is_some(), "adx_14 should be defined");
    assert!(
        s.volatility.mass_index_25.is_some(),
        "mass_index should be defined"
    );
    assert!(
        s.volatility.ttm_squeeze_on.is_some(),
        "ttm_squeeze_on should be defined"
    );
    assert!(s.momentum.kvo_34_55.is_some(), "kvo should be defined");
    assert!(
        s.trend.mama.is_some(),
        "mama should be defined after warmup"
    );
    assert!(last.vwma.is_some(), "vwma should be defined");
    assert!(
        last.vp_poc.is_some(),
        "volume profile POC should be defined when vp_enabled and enough bars"
    );
}
