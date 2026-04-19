use anyhow::{Result, bail};

use crate::config::StrategyConfig;
use crate::domain::{Candle, MacroEvent};
use crate::indicators::{
    ad_line_series, adx_series, aggregate_to_higher_tf, alma_series, aroon_series, atr_series,
    awesome_oscillator_series, bollinger_bandwidth_series, bollinger_pct_b_series,
    bollinger_series, candlestick_pattern_series, cci_series, chaikin_oscillator_series,
    chandelier_exit_series, cmf_series, cmo_series, dema_series, donchian_series, elder_ray_series,
    ema_series, force_index_series, hist_vol_log_returns_series, hull_ma_series, ichimoku_series,
    kama_series, keltner_series, kst_series, kvo_series, linear_regression_slope_series,
    macd_series, mama_fama_series, mass_index_series, mcginley_series, mfi_series, nvi_pvi_series,
    obv_series, parabolic_sar_series, pivot_classic_series, pivot_fib_series, ppo_series,
    pvo_series, roc_series, rolling_median, rsi_series, sma_series, stochastic_rsi_series,
    stochastic_series, supertrend_series, tema_series, trix_series, tsi_series, ttm_squeeze_series,
    ultimate_oscillator_series, vidya_series, volume_ema_series, volume_profile_zones,
    volume_sma_series, vortex_series, vwap_bands_series, vwma_series, williams_r_series,
    wma_series, zscore_series,
};

use super::data::{PreparedCandle, PreparedDataset};
use super::snapshot::{
    CandlestickPatternSnapshot, DirectionalSnapshot, IchimokuSnapshot, IndicatorSnapshot,
    MomentumSnapshot, PivotClassicSnapshot, PivotFibSnapshot, TrendSnapshot, VolatilitySnapshot,
    VolumeSnapshot,
};

impl PreparedDataset {
    pub fn build(
        config: &StrategyConfig,
        candles: Vec<Candle>,
        macro_events: Vec<MacroEvent>,
    ) -> Result<Self> {
        let min_candles = config.vwma_lookback.max(if config.vp_enabled {
            config.vp_lookback_bars
        } else {
            1
        });
        if candles.len() < min_candles {
            bail!(
                "need at least {} candles to compute indicators (vwma / volume profile)",
                min_candles
            );
        }

        let higher_tf_candles = if config.higher_tf_factor > 1 {
            aggregate_to_higher_tf(&candles, config.higher_tf_factor).ok()
        } else {
            None
        };

        let closes: Vec<f64> = candles.iter().map(|candle| candle.close).collect();
        let ema_fast = ema_series(&closes, config.ema_fast_period);
        let ema_slow = ema_series(&closes, config.ema_slow_period);
        let atr_series_data = atr_series(&candles, config.atr_period);
        let vwma = vwma_series(&candles, config.vwma_lookback);

        let deltas: Vec<f64> = candles
            .iter()
            .map(|candle| candle.inferred_delta().unwrap_or(0.0))
            .collect();
        let cvd = cumulative_sum(&deltas);
        let cvd_ema3 = ema_series(&cvd, 3);

        let (ema_fast_higher, ema_slow_higher) = if let Some(ref htf) = higher_tf_candles {
            let closes_htf: Vec<f64> = htf.iter().map(|c| c.close).collect();
            (
                ema_series(&closes_htf, config.ema_fast_period),
                ema_series(&closes_htf, config.ema_slow_period),
            )
        } else {
            (Vec::new(), Vec::new())
        };

        let atr_pct_values: Vec<f64> = candles
            .iter()
            .enumerate()
            .map(|(index, candle)| match atr_series_data[index] {
                Some(atr) if candle.close > 0.0 => atr / candle.close,
                _ => 0.0,
            })
            .collect();
        let atr_pct_baseline = rolling_median(&atr_pct_values, config.vol_baseline_lookback_bars);

        // Library indicators (one-shot series for whole history)
        let rsi_14 = rsi_series(&closes, 14);
        let macd = macd_series(&closes, 12, 26, 9);
        let bb = bollinger_series(&closes, 20, 2.0);
        let stoch = stochastic_series(&candles, 14, 3);
        let sma_20 = sma_series(&closes, 20);
        let sma_50 = sma_series(&closes, 50);
        let sma_200 = sma_series(&closes, 200);
        let obv = obv_series(&candles);
        let adx = adx_series(&candles, 14);
        let cci_20 = cci_series(&candles, 20);
        let williams_r_14 = williams_r_series(&candles, 14);
        let roc_10 = roc_series(&closes, 10);
        let mfi_14 = mfi_series(&candles, 14);
        let ultosc = ultimate_oscillator_series(&candles);
        let tsi_25_13 = tsi_series(&closes, 25, 13);
        let ema_20 = ema_series(&closes, 20);
        let wma_20 = wma_series(&closes, 20);
        let hull_9 = hull_ma_series(&closes, 9);
        let keltner = keltner_series(&candles, 20, 14, 2.0);
        let donchian = donchian_series(&candles, 20);
        let aroon = aroon_series(&candles, 25);

        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let ad_line = ad_line_series(&candles);
        let cmf_20 = cmf_series(&candles, 20);
        let volume_sma_20 = volume_sma_series(&candles, 20);
        let volume_ema_20 = volume_ema_series(&candles, 20);
        let bb_pct_b = bollinger_pct_b_series(&bb, &closes);
        let bb_bandwidth = bollinger_bandwidth_series(&bb);
        let (stoch_rsi_k, stoch_rsi_d) = stochastic_rsi_series(&closes, 14, 14, 3, 3);
        let awesome = awesome_oscillator_series(&candles);
        let ppo = ppo_series(&closes, 12, 26, 9);
        let psar = parabolic_sar_series(&highs, &lows, &closes, 0.02, 0.02, 0.2);
        let supertrend = supertrend_series(&candles, 10, 3.0);
        let vwap_bars = vwap_bands_series(
            &candles,
            config.vwap_anchor_mode,
            config.vwap_rolling_bars,
            config.vwap_include_current_bar,
        );
        let ichimoku = ichimoku_series(&candles);
        let pivot_classic = pivot_classic_series(&candles);
        let pivot_fib = pivot_fib_series(&candles);
        let dema_20 = dema_series(&closes, 20);
        let tema_20 = tema_series(&closes, 20);
        let mcginley_14 = mcginley_series(&closes, 14);
        let kst = kst_series(&closes);
        let (elder_bull, elder_bear) = elder_ray_series(&candles, 13);
        let mass_index_25 = mass_index_series(&candles, 9, 25);

        let vols: Vec<f64> = candles.iter().map(|c| c.volume).collect();
        let cmo_14 = cmo_series(&closes, 14);
        let (trix_15, trix_signal_9) = trix_series(&closes, 15, 9);
        let (kvo_line, kvo_signal) = kvo_series(&candles, 34, 55, 13);
        let chaikin_osc = chaikin_oscillator_series(&ad_line, 3, 10);
        let pvo = pvo_series(&vols, 12, 26, 9);
        let force_13 = force_index_series(&candles, 13);
        let (nvi, pvi) = nvi_pvi_series(&closes, &vols);
        let vortex_14 = vortex_series(&candles, 14);
        let ttm_sq = ttm_squeeze_series(&candles);
        let chandelier = chandelier_exit_series(&candles, 22, 14, 3.0);
        let kama_er10 = kama_series(&closes, 10);
        let alma_20 = alma_series(&closes, 20, 0.85, 6.0);
        let vidya_14 = vidya_series(&closes, 14);
        let mama_fama = mama_fama_series(&candles, 0.5, 0.05);
        let lr_slope_20 = linear_regression_slope_series(&closes, 20);
        let zscore_20 = zscore_series(&closes, 20);
        let hist_vol_20 = hist_vol_log_returns_series(&closes, 20);
        let patterns = candlestick_pattern_series(&candles);

        let mut hour_pointer = 0usize;
        let mut frames = Vec::with_capacity(candles.len());
        for index in 0..candles.len() {
            if let Some(ref htf) = higher_tf_candles {
                while hour_pointer + 1 < htf.len()
                    && htf[hour_pointer + 1].close_time <= candles[index].close_time
                {
                    hour_pointer += 1;
                }
            }

            let hour_ready = higher_tf_candles.as_ref().and_then(|htf| {
                htf.get(hour_pointer)
                    .filter(|hc| hc.close_time <= candles[index].close_time)
            });

            let atr_pct = atr_series_data[index].map(|atr| atr / candles[index].close);
            let baseline = atr_pct_baseline[index];
            let vol_ratio = match (atr_pct, baseline) {
                (Some(current), Some(base)) if base > 0.0 => Some(current / base),
                _ => None,
            };

            let (vp_val, vp_poc, vp_vah) = if config.vp_enabled
                && index + 1 >= config.vp_lookback_bars
                && config.vp_bin_count >= 2
            {
                match volume_profile_zones(
                    &candles,
                    index,
                    config.vp_lookback_bars,
                    config.vp_bin_count,
                    config.vp_value_area_ratio,
                ) {
                    Some(z) => (Some(z.val), Some(z.poc), Some(z.vah)),
                    None => (None, None, None),
                }
            } else {
                (None, None, None)
            };

            let momentum = MomentumSnapshot {
                rsi_14: rsi_14[index],
                macd_line: macd[index].as_ref().map(|m| m.line),
                macd_signal: macd[index].as_ref().map(|m| m.signal),
                macd_hist: macd[index].as_ref().map(|m| m.hist),
                stoch_k: stoch[index].as_ref().map(|s| s.k),
                stoch_d: stoch[index].as_ref().map(|s| s.d),
                stoch_rsi_k: stoch_rsi_k[index],
                stoch_rsi_d: stoch_rsi_d[index],
                cci_20: cci_20[index],
                williams_r_14: williams_r_14[index],
                roc_10: roc_10[index],
                mfi_14: mfi_14[index],
                ultosc_7_14_28: ultosc[index],
                tsi_25_13: tsi_25_13[index],
                awesome_oscillator_5_34: awesome[index],
                ppo_line: ppo[index].as_ref().map(|p| p.line),
                ppo_signal: ppo[index].as_ref().map(|p| p.signal),
                ppo_hist: ppo[index].as_ref().map(|p| p.hist),
                kst: kst[index],
                elder_bull: Some(elder_bull[index]),
                elder_bear: Some(elder_bear[index]),
                cmo_14: cmo_14[index],
                trix_15: trix_15[index],
                trix_signal_9: trix_signal_9[index],
                kvo_34_55: kvo_line[index],
                kvo_signal_13: kvo_signal[index],
                chaikin_oscillator_3_10: chaikin_osc[index],
                pvo_line: pvo[index].as_ref().map(|p| p.line),
                pvo_signal: pvo[index].as_ref().map(|p| p.signal),
                pvo_hist: pvo[index].as_ref().map(|p| p.hist),
                force_index_13: Some(force_13[index]),
            };
            let trend = TrendSnapshot {
                sma_20: sma_20[index],
                sma_50: sma_50[index],
                sma_200: sma_200[index],
                ema_20: Some(ema_20[index]),
                wma_20: wma_20[index],
                hull_9: hull_9[index],
                vwap_session: vwap_bars[index].as_ref().map(|v| v.vwap),
                vwap_upper_1sd: vwap_bars[index].as_ref().map(|v| v.upper_1sd),
                vwap_lower_1sd: vwap_bars[index].as_ref().map(|v| v.lower_1sd),
                vwap_upper_2sd: vwap_bars[index].as_ref().map(|v| v.upper_2sd),
                vwap_lower_2sd: vwap_bars[index].as_ref().map(|v| v.lower_2sd),
                dema_20: dema_20.get(index).copied(),
                tema_20: tema_20.get(index).copied(),
                mcginley_14: mcginley_14.get(index).copied(),
                kama_10: Some(kama_er10[index]),
                alma_20: alma_20[index],
                vidya_14: Some(vidya_14[index]),
                mama: mama_fama[index].as_ref().map(|m| m.mama),
                fama: mama_fama[index].as_ref().map(|m| m.fama),
                lr_slope_20: lr_slope_20[index],
                price_zscore_20: zscore_20[index],
                hist_vol_logrets_20: hist_vol_20[index],
            };
            let iz = &ichimoku[index];
            let ichimoku_snap = IchimokuSnapshot {
                tenkan_9: iz.tenkan_9,
                kijun_26: iz.kijun_26,
                senkou_a_26: iz.senkou_a_26,
                senkou_b_52: iz.senkou_b_52,
                chikou_close_shifted: iz.chikou_close_shifted,
            };
            let pc = &pivot_classic[index];
            let pivot_classic_snap = PivotClassicSnapshot {
                pivot_p: pc.pivot_p,
                pivot_r1: pc.pivot_r1,
                pivot_r2: pc.pivot_r2,
                pivot_r3: pc.pivot_r3,
                pivot_s1: pc.pivot_s1,
                pivot_s2: pc.pivot_s2,
                pivot_s3: pc.pivot_s3,
            };
            let pf = &pivot_fib[index];
            let pivot_fib_snap = PivotFibSnapshot {
                pivot_p: pf.pivot_p,
                pivot_r1: pf.pivot_r1,
                pivot_r2: pf.pivot_r2,
                pivot_r3: pf.pivot_r3,
                pivot_s1: pf.pivot_s1,
                pivot_s2: pf.pivot_s2,
                pivot_s3: pf.pivot_s3,
            };
            let volatility = VolatilitySnapshot {
                bb_middle_20: bb[index].as_ref().map(|b| b.middle),
                bb_upper_20: bb[index].as_ref().map(|b| b.upper),
                bb_lower_20: bb[index].as_ref().map(|b| b.lower),
                bb_pct_b_20: bb_pct_b[index],
                bb_bandwidth_20: bb_bandwidth[index],
                keltner_middle_20: keltner[index].as_ref().map(|k| k.middle),
                keltner_upper_20: keltner[index].as_ref().map(|k| k.upper),
                keltner_lower_20: keltner[index].as_ref().map(|k| k.lower),
                donchian_upper_20: donchian[index].as_ref().map(|d| d.upper),
                donchian_lower_20: donchian[index].as_ref().map(|d| d.lower),
                donchian_mid_20: donchian[index].as_ref().map(|d| d.mid),
                supertrend_10_3: supertrend[index].as_ref().map(|s| s.line),
                supertrend_long: supertrend[index].as_ref().map(|s| s.long),
                mass_index_25: mass_index_25[index],
                pivot_classic: pivot_classic_snap,
                pivot_fib: pivot_fib_snap,
                ttm_squeeze_on: ttm_sq[index].as_ref().map(|t| t.squeezed),
                ttm_squeeze_momentum: ttm_sq[index].as_ref().and_then(|t| t.momentum),
                chandelier_long_stop_22_3: chandelier[index].as_ref().map(|c| c.long_stop),
                chandelier_short_stop_22_3: chandelier[index].as_ref().map(|c| c.short_stop),
            };
            let directional = DirectionalSnapshot {
                adx_14: adx[index].as_ref().map(|a| a.adx),
                di_plus: adx[index].as_ref().map(|a| a.di_plus),
                di_minus: adx[index].as_ref().map(|a| a.di_minus),
                aroon_up_25: aroon[index].as_ref().map(|a| a.up),
                aroon_down_25: aroon[index].as_ref().map(|a| a.down),
                psar: psar[index].as_ref().map(|p| p.sar),
                psar_trend_long: psar[index].as_ref().map(|p| p.is_long),
                vortex_vi_plus_14: vortex_14[index].as_ref().map(|v| v.vi_plus),
                vortex_vi_minus_14: vortex_14[index].as_ref().map(|v| v.vi_minus),
            };
            let volume = VolumeSnapshot {
                obv: Some(obv[index]),
                ad_line: Some(ad_line[index]),
                cmf_20: cmf_20[index],
                volume_sma_20: volume_sma_20[index],
                volume_ema_20: Some(volume_ema_20[index]),
                nvi: Some(nvi[index]),
                pvi: Some(pvi[index]),
            };
            let pat = &patterns[index];
            let patterns_snap = CandlestickPatternSnapshot {
                bull_engulfing: pat.bull_engulfing,
                bear_engulfing: pat.bear_engulfing,
                hammer: pat.hammer,
                shooting_star: pat.shooting_star,
                doji: pat.doji,
            };

            frames.push(PreparedCandle {
                candle: candles[index].clone(),
                ema_fast: Some(ema_fast[index]),
                ema_slow: Some(ema_slow[index]),
                ema_fast_higher: hour_ready.map(|_| ema_fast_higher[hour_pointer]),
                ema_slow_higher: hour_ready.map(|_| ema_slow_higher[hour_pointer]),
                vwma: vwma[index],
                atr: atr_series_data[index],
                atr_pct,
                atr_pct_baseline: baseline,
                vol_ratio,
                cvd_ema3: Some(cvd_ema3[index]),
                cvd_ema3_slope: if index == 0 {
                    Some(0.0)
                } else {
                    Some(cvd_ema3[index] - cvd_ema3[index - 1])
                },
                vp_val,
                vp_poc,
                vp_vah,
                indicator_snapshot: IndicatorSnapshot {
                    momentum,
                    trend,
                    ichimoku: ichimoku_snap,
                    volatility,
                    directional,
                    volume,
                    patterns: patterns_snap,
                },
            });
        }

        Ok(Self {
            frames,
            macro_events,
        })
    }
}

fn cumulative_sum(values: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(values.len());
    let mut total = 0.0;
    for value in values {
        total += value;
        result.push(total);
    }
    result
}
