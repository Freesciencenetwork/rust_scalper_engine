use crate::config::StrategyConfig;
use crate::market_data::PreparedDataset;

pub fn active(
    index: usize,
    dataset: &PreparedDataset,
    entry_price: f64,
    config: &StrategyConfig,
) -> bool {
    let Some(atr) = dataset.frames[index].atr else {
        return false;
    };
    if index < 4 {
        return false;
    }

    let start = index.saturating_sub(config.runway_lookback);
    let mut nearest_barrier: Option<f64> = None;
    for candidate in (start + 2)..index.saturating_sub(1) {
        if candidate + 2 > index {
            break;
        }
        let high = dataset.frames[candidate].candle.high;
        let left_one = dataset.frames[candidate - 1].candle.high;
        let left_two = dataset.frames[candidate - 2].candle.high;
        let right_one = dataset.frames[candidate + 1].candle.high;
        let right_two = dataset.frames[candidate + 2].candle.high;
        let is_local_high =
            high > left_one && high > left_two && high >= right_one && high >= right_two;
        if is_local_high && high > entry_price {
            nearest_barrier = match nearest_barrier {
                Some(existing) => Some(existing.min(high)),
                None => Some(high),
            };
        }
    }

    let required_runway = config.target_atr_multiple * atr;
    matches!(nearest_barrier, Some(barrier) if barrier - entry_price < required_runway)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::active;
    use crate::config::StrategyConfig;
    use crate::domain::Candle;
    use crate::market_data::{snapshot::IndicatorSnapshot, PreparedCandle, PreparedDataset};

    fn frame_at(minute: i64, high: f64, close: f64, atr: f64) -> PreparedCandle {
        PreparedCandle {
            candle: Candle {
                close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                    + Duration::minutes(minute),
                open: close - 1.0,
                high,
                low: close - 2.0,
                close,
                volume: 1.0,
                buy_volume: None,
                sell_volume: None,
                delta: None,
            },
            ema_fast: None,
            ema_slow: None,
            ema_fast_higher: None,
            ema_slow_higher: None,
            vwma: None,
            atr: Some(atr),
            atr_pct: None,
            atr_pct_baseline: None,
            vol_ratio: None,
            cvd_ema3: None,
            cvd_ema3_slope: None,
            vp_val: None,
            vp_poc: None,
            vp_vah: None,
            indicator_snapshot: IndicatorSnapshot::default(),
        }
    }

    #[test]
    fn no_runway_uses_target_distance_not_stop_distance() {
        let frames = vec![
            frame_at(0, 100.0, 99.0, 10.0),
            frame_at(15, 101.0, 100.0, 10.0),
            frame_at(30, 102.0, 101.0, 10.0),
            frame_at(45, 120.0, 103.0, 10.0),
            frame_at(60, 104.0, 103.5, 10.0),
            frame_at(75, 103.0, 102.5, 10.0),
            frame_at(90, 105.0, 104.0, 10.0),
        ];
        let dataset = PreparedDataset {
            frames,
            macro_events: Vec::new(),
        };
        let config = StrategyConfig {
            runway_lookback: 6,
            stop_atr_multiple: 2.0,
            target_atr_multiple: 3.0,
            ..StrategyConfig::default()
        };

        assert!(active(6, &dataset, 104.0, &config));
    }
}
