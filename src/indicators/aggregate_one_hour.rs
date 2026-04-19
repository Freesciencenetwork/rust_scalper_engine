use std::collections::BTreeMap;

use anyhow::{Result, bail};
use chrono::{DateTime, Duration, Timelike, Utc};

use crate::domain::Candle;

pub fn aggregate_15m_to_1h(candles: &[Candle]) -> Result<Vec<Candle>> {
    let mut groups: BTreeMap<DateTime<Utc>, Vec<Candle>> = BTreeMap::new();

    for candle in candles {
        let close_time = candle.close_time;
        let hour_floor = close_time
            .date_naive()
            .and_hms_opt(close_time.hour(), 0, 0)
            .expect("valid hour");
        let hour_floor = DateTime::<Utc>::from_naive_utc_and_offset(hour_floor, Utc);
        let hour_close = if close_time.minute() == 0 && close_time.second() == 0 {
            hour_floor
        } else {
            hour_floor + Duration::hours(1)
        };
        groups.entry(hour_close).or_default().push(candle.clone());
    }

    let mut aggregated = Vec::new();
    for (hour_close, mut group) in groups {
        if group.len() != 4 {
            continue;
        }
        group.sort_by_key(|candle| candle.close_time);
        if !is_complete_hour_group(hour_close, &group) {
            continue;
        }

        let first = group.first().expect("non-empty group");
        let last = group.last().expect("non-empty group");
        let high = group
            .iter()
            .map(|candle| candle.high)
            .reduce(f64::max)
            .expect("group has highs");
        let low = group
            .iter()
            .map(|candle| candle.low)
            .reduce(f64::min)
            .expect("group has lows");
        let volume: f64 = group.iter().map(|candle| candle.volume).sum();
        let buy_volume = sum_optional(group.iter().map(|candle| candle.buy_volume));
        let sell_volume = sum_optional(group.iter().map(|candle| candle.sell_volume));
        let delta = sum_optional(group.iter().map(|candle| candle.inferred_delta()));

        aggregated.push(Candle {
            close_time: hour_close,
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

    if aggregated.is_empty() {
        bail!("unable to derive any complete 1h candles from the supplied 15m data");
    }

    Ok(aggregated)
}

fn is_complete_hour_group(hour_close: DateTime<Utc>, group: &[Candle]) -> bool {
    let expected_offsets = [45_i64, 30, 15, 0];
    group
        .iter()
        .zip(expected_offsets)
        .all(|(candle, offset_minutes)| {
            candle.close_time == hour_close - Duration::minutes(offset_minutes)
        })
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
    use chrono::{Duration, TimeZone, Timelike, Utc};

    use super::aggregate_15m_to_1h;
    use crate::domain::Candle;

    fn candle_at(minute: i64, close: f64) -> Candle {
        Candle {
            close_time: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                + Duration::minutes(minute),
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
    fn aggregates_four_fifteen_minute_candles_into_one_hour() {
        let candles = vec![
            candle_at(15, 100.0),
            candle_at(30, 101.0),
            candle_at(45, 102.0),
            candle_at(60, 103.0),
        ];
        let aggregated = aggregate_15m_to_1h(&candles).expect("aggregation");
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].close, 103.0);
        assert_eq!(aggregated[0].close_time.hour(), 1);
    }

    #[test]
    fn sorts_group_before_aggregating_open_and_close() {
        let candles = vec![
            candle_at(45, 102.0),
            candle_at(15, 100.0),
            candle_at(60, 103.0),
            candle_at(30, 101.0),
        ];

        let aggregated = aggregate_15m_to_1h(&candles).expect("aggregation");
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].open, 99.0);
        assert_eq!(aggregated[0].close, 103.0);
    }

    #[test]
    fn skips_groups_with_missing_or_duplicate_quarter_hours() {
        let candles = vec![
            candle_at(15, 100.0),
            candle_at(15, 101.0),
            candle_at(45, 102.0),
            candle_at(60, 103.0),
        ];

        let err = aggregate_15m_to_1h(&candles).expect_err("expected incomplete group rejection");
        assert!(
            err.to_string()
                .contains("unable to derive any complete 1h candles")
        );
    }
}
