use anyhow::{Result, bail};

use crate::config::StrategyConfig;
use crate::domain::{Candle, MacroEvent};
use crate::indicators::{aggregate_15m_to_1h, atr_series, ema_series, rolling_median, vwma_series};

use super::data::{PreparedCandle, PreparedDataset};

impl PreparedDataset {
    pub fn build(
        config: &StrategyConfig,
        candles_15m: Vec<Candle>,
        macro_events: Vec<MacroEvent>,
    ) -> Result<Self> {
        if candles_15m.len() < config.vwma_lookback {
            bail!(
                "need at least {} 15m candles to compute the v1 indicators",
                config.vwma_lookback
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
