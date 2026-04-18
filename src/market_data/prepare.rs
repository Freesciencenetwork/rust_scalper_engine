use anyhow::{Result, bail};

use crate::config::StrategyConfig;
use crate::domain::{Candle, MacroEvent};
use crate::indicators::{
    ad_line_series, adx_series, aggregate_15m_to_1h, aroon_series, atr_series,
    awesome_oscillator_series, bollinger_bandwidth_series, bollinger_pct_b_series, bollinger_series,
    cci_series, cmf_series, dema_series, donchian_series, elder_ray_series, ema_series,
    hull_ma_series, ichimoku_series, keltner_series, kst_series, macd_series, mass_index_series,
    mcginley_series, mfi_series, obv_series, parabolic_sar_series, pivot_classic_series,
    pivot_fib_series, ppo_series, roc_series, rolling_median, rsi_series, sma_series,
    stochastic_rsi_series, stochastic_series, supertrend_series, tema_series, tsi_series,
    ultimate_oscillator_series, volume_ema_series, volume_profile_zones, volume_sma_series,
    vwap_bands_series, vwma_series, williams_r_series, wma_series,
};

use super::data::{PreparedCandle, PreparedDataset};
use super::snapshot::{
    DirectionalSnapshot, IchimokuSnapshot, IndicatorSnapshot, MomentumSnapshot, PivotClassicSnapshot,
    PivotFibSnapshot, TrendSnapshot, VolatilitySnapshot, VolumeSnapshot,
};

impl PreparedDataset {
    pub fn build(
        config: &StrategyConfig,
        candles_15m: Vec<Candle>,
        macro_events: Vec<MacroEvent>,
    ) -> Result<Self> {
        let min_candles = config
            .vwma_lookback
            .max(if config.vp_enabled {
                config.vp_lookback_bars
            } else {
                1
            });
        if candles_15m.len() < min_candles {
            bail!(
                "need at least {} 15m candles to compute indicators (vwma / volume profile)",
                min_candles
            );
        }

        let one_hour = aggregate_15m_to_1h(&candles_15m)?;
        let closes_15m: Vec<f64> = candles_15m.iter().map(|candle| candle.close).collect();
        let ema_fast_15m = ema_series(&closes_15m, config.ema_fast_period);
        let ema_slow_15m = ema_series(&closes_15m, config.ema_slow_period);
        let atr_15m = atr_series(&candles_15m, config.atr_period);
        let vwma_15m = vwma_series(&candles_15m, config.vwma_lookback);

        let deltas: Vec<f64> = candles_15m
            .iter()
            .map(|candle| candle.inferred_delta().unwrap_or(0.0))
            .collect();
        let cvd = cumulative_sum(&deltas);
        let cvd_ema3 = ema_series(&cvd, 3);

        let closes_1h: Vec<f64> = one_hour.iter().map(|candle| candle.close).collect();
        let ema_fast_1h = ema_series(&closes_1h, config.ema_fast_period);
        let ema_slow_1h = ema_series(&closes_1h, config.ema_slow_period);

        let atr_pct_values: Vec<f64> = candles_15m
            .iter()
            .enumerate()
            .map(|(index, candle)| match atr_15m[index] {
                Some(atr) if candle.close > 0.0 => atr / candle.close,
                _ => 0.0,
            })
            .collect();
        let atr_pct_baseline = rolling_median(&atr_pct_values, config.vol_baseline_lookback_bars);

        // Library indicators (one-shot series for whole history)
        let rsi_14 = rsi_series(&closes_15m, 14);
        let macd = macd_series(&closes_15m, 12, 26, 9);
        let bb = bollinger_series(&closes_15m, 20, 2.0);
        let stoch = stochastic_series(&candles_15m, 14, 3);
        let sma_20 = sma_series(&closes_15m, 20);
        let sma_50 = sma_series(&closes_15m, 50);
        let sma_200 = sma_series(&closes_15m, 200);
        let obv = obv_series(&candles_15m);
        let adx = adx_series(&candles_15m, 14);
        let cci_20 = cci_series(&candles_15m, 20);
        let williams_r_14 = williams_r_series(&candles_15m, 14);
        let roc_10 = roc_series(&closes_15m, 10);
        let mfi_14 = mfi_series(&candles_15m, 14);
        let ultosc = ultimate_oscillator_series(&candles_15m);
        let tsi_25_13 = tsi_series(&closes_15m, 25, 13);
        let ema_20 = ema_series(&closes_15m, 20);
        let wma_20 = wma_series(&closes_15m, 20);
        let hull_9 = hull_ma_series(&closes_15m, 9);
        let keltner = keltner_series(&candles_15m, 20, 14, 2.0);
        let donchian = donchian_series(&candles_15m, 20);
        let aroon = aroon_series(&candles_15m, 25);

        let highs_15m: Vec<f64> = candles_15m.iter().map(|c| c.high).collect();
        let lows_15m: Vec<f64> = candles_15m.iter().map(|c| c.low).collect();
        let ad_line = ad_line_series(&candles_15m);
        let cmf_20 = cmf_series(&candles_15m, 20);
        let volume_sma_20 = volume_sma_series(&candles_15m, 20);
        let volume_ema_20 = volume_ema_series(&candles_15m, 20);
        let bb_pct_b = bollinger_pct_b_series(&bb, &closes_15m);
        let bb_bandwidth = bollinger_bandwidth_series(&bb);
        let (stoch_rsi_k, stoch_rsi_d) = stochastic_rsi_series(&closes_15m, 14, 14, 3, 3);
        let awesome = awesome_oscillator_series(&candles_15m);
        let ppo = ppo_series(&closes_15m, 12, 26, 9);
        let psar = parabolic_sar_series(&highs_15m, &lows_15m, &closes_15m, 0.02, 0.02, 0.2);
        let supertrend = supertrend_series(&candles_15m, 10, 3.0);
        let vwap_bars = vwap_bands_series(
            &candles_15m,
            config.vwap_anchor_mode,
            config.vwap_rolling_bars,
            config.vwap_include_current_bar,
        );
        let ichimoku = ichimoku_series(&candles_15m);
        let pivot_classic = pivot_classic_series(&candles_15m);
        let pivot_fib = pivot_fib_series(&candles_15m);
        let dema_20 = dema_series(&closes_15m, 20);
        let tema_20 = tema_series(&closes_15m, 20);
        let mcginley_14 = mcginley_series(&closes_15m, 14);
        let kst = kst_series(&closes_15m);
        let (elder_bull, elder_bear) = elder_ray_series(&candles_15m, 13);
        let mass_index_25 = mass_index_series(&candles_15m, 9, 25);

        let mut hour_pointer = 0usize;
        let mut frames_15m = Vec::with_capacity(candles_15m.len());
        for index in 0..candles_15m.len() {
            while hour_pointer + 1 < one_hour.len()
                && one_hour[hour_pointer + 1].close_time <= candles_15m[index].close_time
            {
                hour_pointer += 1;
            }

            let hour_ready = one_hour
                .get(hour_pointer)
                .filter(|hour_candle| hour_candle.close_time <= candles_15m[index].close_time);

            let atr_pct = atr_15m[index].map(|atr| atr / candles_15m[index].close);
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
                    &candles_15m,
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
            };
            let directional = DirectionalSnapshot {
                adx_14: adx[index].as_ref().map(|a| a.adx),
                di_plus: adx[index].as_ref().map(|a| a.di_plus),
                di_minus: adx[index].as_ref().map(|a| a.di_minus),
                aroon_up_25: aroon[index].as_ref().map(|a| a.up),
                aroon_down_25: aroon[index].as_ref().map(|a| a.down),
                psar: psar[index].as_ref().map(|p| p.sar),
                psar_trend_long: psar[index].as_ref().map(|p| p.is_long),
            };
            let volume = VolumeSnapshot {
                obv: Some(obv[index]),
                ad_line: Some(ad_line[index]),
                cmf_20: cmf_20[index],
                volume_sma_20: volume_sma_20[index],
                volume_ema_20: Some(volume_ema_20[index]),
            };

            frames_15m.push(PreparedCandle {
                candle: candles_15m[index].clone(),
                ema_fast_15m: Some(ema_fast_15m[index]),
                ema_slow_15m: Some(ema_slow_15m[index]),
                ema_fast_1h: hour_ready.map(|_| ema_fast_1h[hour_pointer]),
                ema_slow_1h: hour_ready.map(|_| ema_slow_1h[hour_pointer]),
                vwma_15m: vwma_15m[index],
                atr_15m: atr_15m[index],
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
                },
            });
        }

        Ok(Self {
            frames_15m,
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
