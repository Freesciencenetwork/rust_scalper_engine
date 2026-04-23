#![allow(non_snake_case)] // Crate name is binance_BTC.

use anyhow::{Context, Result};
use binance_BTC::{
    Candle, ConfigOverrides, DecisionMachine, ExecutionAssumptions, MachineRequest, RuntimeState,
    StrategyBacktestRequest, StrategyConfig, strategy_engine_for,
};
use binance_BTC::historical_data::{BundledBtcUsd1m, load_btcusd_1m};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};

const CHUNK_DAYS: i64 = 180;
const WARMUP_DAYS: i64 = 2;

fn main() -> Result<()> {
    let strategy_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "default".to_string());
    let machine = DecisionMachine::new(StrategyConfig::default());

    let first_day = csv_first_day()?;
    let last_day = csv_last_day()?;

    println!(
        "Dataset coverage: {} -> {}",
        first_day.format("%Y-%m-%d"),
        last_day.format("%Y-%m-%d")
    );

    let signal_stats = count_allowed_signals(&machine, &strategy_id, first_day, last_day)?;
    println!();
    println!("Step 0 — Signal Frequency (15m resampled, strategy={strategy_id})");
    println!("  chunks processed      : {}", signal_stats.chunk_count);
    println!("  counted bars          : {}", signal_stats.counted_bars);
    println!("  allowed decisions     : {}", signal_stats.allowed_count);
    println!("  allowed with trigger  : {}", signal_stats.armable_count);
    println!(
        "  allowed rate          : {:.4}%",
        100.0 * signal_stats.allowed_count as f64 / signal_stats.counted_bars.max(1) as f64
    );
    println!(
        "  armable rate          : {:.4}%",
        100.0 * signal_stats.armable_count as f64 / signal_stats.counted_bars.max(1) as f64
    );

    let smoke_from = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");
    let smoke_to = NaiveDate::from_ymd_opt(2024, 1, 31).expect("valid date");
    let backtest = machine
        .evaluate_backtest(StrategyBacktestRequest {
            machine: MachineRequest {
                candles: aggregate_to_15m(load_btcusd_1m(&BundledBtcUsd1m {
                    from: Some(smoke_from.format("%Y-%m-%d").to_string()),
                    to: Some(smoke_to.format("%Y-%m-%d").to_string()),
                    all: false,
                })?),
                bar_interval: Some("15m".to_string()),
                macro_events: Vec::new(),
                runtime_state: RuntimeState::default(),
                account_equity: None,
                symbol_filters: None,
                config_overrides: Some(ConfigOverrides {
                    strategy_id: Some(strategy_id.clone()),
                    ..Default::default()
                }),
                synthetic_series: None,
                bundled_btcusd_1m: None,
                bundled_resample_interval: None,
            },
            from_index: None,
            to_index: None,
            replay_from: None,
            replay_to: None,
            execution: ExecutionAssumptions::default(),
        })
        .context("evaluate_backtest smoke slice")?;

    println!();
    println!(
        "Step 4 — Backtest Smoke ({} -> {}, strategy={strategy_id})",
        smoke_from.format("%Y-%m-%d"),
        smoke_to.format("%Y-%m-%d")
    );
    println!("  strategy_id           : {}", backtest.strategy_id);
    println!("  trade_count           : {}", backtest.summary.trade_count);
    println!("  win_rate              : {:.4}%", 100.0 * backtest.summary.win_rate);
    println!("  avg_net_r             : {:.6}", backtest.summary.avg_net_r);
    println!("  total_net_r           : {:.6}", backtest.summary.total_net_r);
    println!(
        "  profit_factor         : {}",
        backtest
            .summary
            .profit_factor
            .map(|value| format!("{value:.6}"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!("  max_drawdown_r        : {:.6}", backtest.summary.max_drawdown_r);
    if let Some(first_trade) = backtest.trades.first() {
        println!("  first_trade_signal_ms : {}", first_trade.signal_close_time.timestamp_millis());
        println!("  first_trade_exit      : {:?}", first_trade.exit_reason);
        println!("  first_trade_net_r     : {:.6}", first_trade.net_r);
    } else {
        println!("  first_trade           : none");
    }

    Ok(())
}

#[derive(Default)]
struct SignalStats {
    chunk_count: usize,
    counted_bars: usize,
    allowed_count: usize,
    armable_count: usize,
}

fn count_allowed_signals(
    machine: &DecisionMachine,
    strategy_id: &str,
    first_day: NaiveDate,
    last_day: NaiveDate,
) -> Result<SignalStats> {
    let mut stats = SignalStats::default();
    let mut chunk_start = first_day;

    while chunk_start <= last_day {
        let chunk_end = (chunk_start + Duration::days(CHUNK_DAYS - 1)).min(last_day);
        let load_start = if chunk_start > first_day + Duration::days(WARMUP_DAYS) {
            chunk_start - Duration::days(WARMUP_DAYS)
        } else {
            first_day
        };
        let count_start = utc_day_start(chunk_start);
        let candles_15m = aggregate_to_15m(load_btcusd_1m(&BundledBtcUsd1m {
            from: Some(load_start.format("%Y-%m-%d").to_string()),
            to: Some(chunk_end.format("%Y-%m-%d").to_string()),
            all: false,
        })?);
        let (config, dataset) = machine
            .prepare_dataset(MachineRequest {
                candles: candles_15m,
                bar_interval: Some("15m".to_string()),
                macro_events: Vec::new(),
                runtime_state: RuntimeState::default(),
                account_equity: None,
                symbol_filters: None,
                config_overrides: Some(ConfigOverrides {
                    strategy_id: Some(strategy_id.to_string()),
                    ..Default::default()
                }),
                synthetic_series: None,
                bundled_btcusd_1m: None,
                bundled_resample_interval: None,
            })
            .with_context(|| {
                format!(
                    "prepare_dataset chunk {} -> {}",
                    load_start.format("%Y-%m-%d"),
                    chunk_end.format("%Y-%m-%d")
                )
            })?;

        let mut engine = strategy_engine_for(&config)?;
        let mut fa_cursor = 0usize;
        for (index, frame) in dataset.frames.iter().enumerate() {
            if fa_cursor <= index {
                engine.replay_failed_acceptance_window(fa_cursor, index, &dataset);
                fa_cursor = index.saturating_add(1);
            }
            let close_time = frame.candle.close_time;
            if close_time < count_start {
                continue;
            }
            stats.counted_bars += 1;
            let decision = engine.decide(index, &dataset);
            if decision.allowed {
                stats.allowed_count += 1;
                if decision.trigger_price.is_some() {
                    stats.armable_count += 1;
                }
            }
        }

        stats.chunk_count += 1;
        eprintln!(
            "counted chunk {} -> {}",
            chunk_start.format("%Y-%m-%d"),
            chunk_end.format("%Y-%m-%d")
        );
        chunk_start = chunk_end + Duration::days(1);
    }

    Ok(stats)
}

fn csv_first_day() -> Result<NaiveDate> {
    let path = std::env::var("BTCUSD_1M_CSV")
        .unwrap_or_else(|_| "src/historical_data/btcusd_1-min_data.csv".to_string());
    let file = std::fs::File::open(&path).with_context(|| format!("open {path}"))?;
    let mut lines = std::io::BufRead::lines(std::io::BufReader::new(file));
    let _header = lines.next().context("csv header")??;
    let line = lines.next().context("first csv row")??;
    ts_field_to_day(line.split(',').next().context("timestamp field")?)
}

fn csv_last_day() -> Result<NaiveDate> {
    let path = std::env::var("BTCUSD_1M_CSV")
        .unwrap_or_else(|_| "src/historical_data/btcusd_1-min_data.csv".to_string());
    let data = std::fs::read(&path).with_context(|| format!("read {path}"))?;
    let last_line = data
        .rsplit(|byte| *byte == b'\n')
        .find(|line| !line.is_empty())
        .context("last csv row")?;
    let line = std::str::from_utf8(last_line).context("utf8 last row")?;
    ts_field_to_day(line.split(',').next().context("timestamp field")?)
}

fn ts_field_to_day(ts_field: &str) -> Result<NaiveDate> {
    let ts_sec: i64 = ts_field
        .trim()
        .parse::<f64>()
        .map(|value| value as i64)
        .with_context(|| format!("timestamp {ts_field:?}"))?;
    let dt: DateTime<Utc> = Utc
        .timestamp_opt(ts_sec, 0)
        .single()
        .context("bad timestamp")?;
    Ok(dt.date_naive())
}

fn utc_day_start(day: NaiveDate) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(day.year(), day.month(), day.day(), 0, 0, 0)
        .single()
        .expect("valid day start")
}

fn aggregate_to_15m(candles_1m: Vec<Candle>) -> Vec<Candle> {
    let mut out = Vec::new();
    let mut group: Vec<Candle> = Vec::with_capacity(15);
    let mut current_bucket: Option<i64> = None;

    for candle in candles_1m {
        let ts = candle.close_time.timestamp();
        let bucket = (ts - 1).div_euclid(15 * 60);
        if let Some(active) = current_bucket
            && active != bucket
        {
            if group.len() == 15 {
                out.push(collapse_bucket(&group));
            }
            group.clear();
        }
        current_bucket = Some(bucket);
        group.push(candle);
    }

    if group.len() == 15 {
        out.push(collapse_bucket(&group));
    }

    out
}

fn collapse_bucket(group: &[Candle]) -> Candle {
    let first = &group[0];
    let last = &group[group.len() - 1];
    Candle {
        close_time: last.close_time,
        open: first.open,
        high: group.iter().map(|candle| candle.high).fold(f64::MIN, f64::max),
        low: group.iter().map(|candle| candle.low).fold(f64::MAX, f64::min),
        close: last.close,
        volume: group.iter().map(|candle| candle.volume).sum(),
        buy_volume: None,
        sell_volume: None,
        delta: None,
    }
}
