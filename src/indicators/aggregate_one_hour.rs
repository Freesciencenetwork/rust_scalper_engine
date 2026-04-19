use anyhow::{Result, bail};
use crate::domain::Candle;

/// Aggregate `candles` into higher-timeframe bars by grouping every `factor` consecutive bars.
///
/// Trailing incomplete groups (fewer than `factor` bars) are dropped.
/// Returns an error when `factor` is zero or no complete groups can be formed.
pub fn aggregate_to_higher_tf(candles: &[Candle], factor: usize) -> Result<Vec<Candle>> {
    if factor == 0 {
        bail!("higher_tf_factor must be >= 1");
    }
    if factor == 1 {
        return Ok(candles.to_vec());
    }
    let complete_groups = candles.len() / factor;
    if complete_groups == 0 {
        bail!(
            "not enough candles ({}) to form even one higher-TF bar (factor={})",
            candles.len(),
            factor
        );
    }
    let mut aggregated = Vec::with_capacity(complete_groups);
    for g in 0..complete_groups {
        let group = &candles[g * factor..(g + 1) * factor];
        let first = &group[0];
        let last = &group[group.len() - 1];
        let high = group.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
        let low = group.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let volume: f64 = group.iter().map(|c| c.volume).sum();
        let buy_volume = sum_optional(group.iter().map(|c| c.buy_volume));
        let sell_volume = sum_optional(group.iter().map(|c| c.sell_volume));
        let delta = sum_optional(group.iter().map(|c| c.inferred_delta()));
        aggregated.push(Candle {
            close_time: last.close_time,
            open: first.open,
            high,
            low,
            close: last.close,
            volume,
            buy_volume,
            sell_volume,
            delta,
        });
    }
    Ok(aggregated)
}

fn sum_optional<I>(values: I) -> Option<f64>
where
    I: IntoIterator<Item = Option<f64>>,
{
    let mut total = 0.0;
    let mut count = 0;
    for number in values.into_iter().flatten() {
        total += number;
        count += 1;
    }
    (count > 0).then_some(total)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::aggregate_to_higher_tf;
    use crate::domain::Candle;

    fn make_candle(i: i64, close: f64) -> Candle {
        Candle {
            close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                + Duration::minutes(i * 15),
            open: close - 1.0,
            high: close + 1.0,
            low: close - 2.0,
            close,
            volume: 10.0,
            buy_volume: Some(6.0),
            sell_volume: Some(4.0),
            delta: None,
        }
    }

    #[test]
    fn groups_four_bars_into_one() {
        let candles: Vec<Candle> = (0..4).map(|i| make_candle(i, 100.0 + i as f64)).collect();
        let result = aggregate_to_higher_tf(&candles, 4).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].open, candles[0].open);
        assert_eq!(result[0].close, candles[3].close);
        assert_eq!(result[0].close_time, candles[3].close_time);
    }

    #[test]
    fn drops_trailing_incomplete_group() {
        let candles: Vec<Candle> = (0..9).map(|i| make_candle(i, 100.0 + i as f64)).collect();
        let result = aggregate_to_higher_tf(&candles, 4).expect("ok");
        // 9 / 4 = 2 complete groups; trailing 1 bar dropped
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn factor_one_returns_clone() {
        let candles: Vec<Candle> = (0..5).map(|i| make_candle(i, 100.0)).collect();
        let result = aggregate_to_higher_tf(&candles, 1).expect("ok");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn factor_zero_errors() {
        let candles: Vec<Candle> = (0..4).map(|i| make_candle(i, 100.0)).collect();
        assert!(aggregate_to_higher_tf(&candles, 0).is_err());
    }

    #[test]
    fn too_few_candles_for_factor_errors() {
        let candles: Vec<Candle> = (0..3).map(|i| make_candle(i, 100.0)).collect();
        assert!(aggregate_to_higher_tf(&candles, 4).is_err());
    }

    #[test]
    fn high_low_volume_aggregated_correctly() {
        let candles = vec![
            make_candle(0, 100.0), // high=101, low=98
            make_candle(1, 105.0), // high=106, low=103
            make_candle(2, 95.0),  // high=96, low=93
            make_candle(3, 102.0), // high=103, low=100
        ];
        let result = aggregate_to_higher_tf(&candles, 4).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].high, 106.0);
        assert_eq!(result[0].low, 93.0);
        assert_eq!(result[0].volume, 40.0);
    }
}
