//! Stream **all** Binance Spot **BTCUSDT** **1m** klines into `src/historical_data/request.json`
//! using `binance_spot_candles::adapters::binance::fetch_klines` (same stack as `binance-fetch`).
//! Output uses the JSON key **`candles`** (not upstream’s legacy **`candles_15m`** name).
//!
//! Pagination: each call returns up to **1000** bars; `startTime` advances to `last.close_time + 1ms`.
//!
//! ```text
//! cargo run --release --bin fetch_max_btcusdt_1m
//! ```
//!
//! Optional env (defaults in parentheses): **`BINANCE_BASE_URL`** (`https://api.binance.com`),
//! **`BINANCE_SYMBOL`** (`BTCUSDT`), **`BINANCE_INTERVAL`** (`1m`),
//! **`BINANCE_START_OPEN_MS`** (`1_502_942_400_000` = first 1m bar Binance serves for this pair),
//! **`BINANCE_SLEEP_SEC`** (`0.02`), **`FETCH_OUT`** (`src/historical_data/request.json` under the manifest dir).

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use binance_spot_candles::adapters::binance::{fetch_klines, http_client};
use tokio::time::{Duration as TokioDuration, sleep};

const DEFAULT_BASE: &str = "https://api.binance.com";
/// Open time (ms) of the earliest **BTCUSDT** **1m** kline Binance returns for `startTime=0`.
const DEFAULT_START_OPEN_MS: i64 = 1_502_942_400_000;

#[tokio::main]
async fn main() -> Result<()> {
    let base = env::var("BINANCE_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let symbol = env::var("BINANCE_SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string());
    let interval = env::var("BINANCE_INTERVAL").unwrap_or_else(|_| "1m".to_string());
    let start_open_ms: i64 = env::var("BINANCE_START_OPEN_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_START_OPEN_MS);
    let sleep_sec: f64 = env::var("BINANCE_SLEEP_SEC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.02);
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_rel =
        env::var("FETCH_OUT").unwrap_or_else(|_| "src/historical_data/request.json".to_string());
    let out_path = manifest.join(&out_rel);
    let partial_name = out_path.file_name().map_or_else(
        || "request.json.partial".into(),
        |s| {
            let mut o = s.to_os_string();
            o.push(".partial");
            o
        },
    );
    let partial_path = out_path
        .parent()
        .unwrap_or(manifest.as_path())
        .join(partial_name);

    if let Some(parent) = partial_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    let client = http_client().context("HTTP client")?;
    let mut next_start: Option<i64> = Some(start_open_ms);
    let mut batches = 0_u64;
    let mut candles = 0_u64;
    let t0 = Instant::now();

    let file = File::create(&partial_path)
        .with_context(|| format!("create {}", partial_path.display()))?;
    let mut w = BufWriter::new(file);
    writeln!(
        w,
        "{{\n  \"bar_interval\": {interval:?},\n  \"candles\": [\n"
    )?;
    let mut first = true;

    while let Some(start_ms) = next_start {
        let batch = fetch_klines(
            &client,
            &base,
            &symbol,
            &interval,
            1000,
            Some(start_ms),
            None,
        )
        .await
        .with_context(|| format!("fetch_klines startTime={start_ms}"))?;

        if batch.is_empty() {
            break;
        }

        for c in &batch {
            if !first {
                writeln!(w, ",")?;
            }
            first = false;
            write!(
                w,
                "{}",
                serde_json::to_string(c).context("serialize candle")?
            )?;
            candles += 1;
        }

        batches += 1;
        if batches.is_multiple_of(50) {
            eprintln!(
                "{batches} batches, {candles} candles, elapsed {:.0}s",
                t0.elapsed().as_secs_f64()
            );
        }

        if batch.len() < 1000 {
            break;
        }

        let last = batch.last().expect("len checked");
        // Next kline's open time is the millisecond after this bar's inclusive `close_time`.
        let close_ms = last.close_time.timestamp_millis();
        next_start = Some(close_ms.saturating_add(1));

        sleep(TokioDuration::from_secs_f64(sleep_sec)).await;
    }

    writeln!(
        w,
        "\n  ],\n  \"macro_events\": [],\n  \"runtime_state\": {{\n    \"realized_net_r_today\": 0.0,\n    \"halt_new_entries_flag\": 0\n  }},\n  \"account_equity\": null,\n  \"symbol_filters\": null,\n  \"rustyfish_overlay\": null\n}}\n"
    )?;
    w.flush().context("flush")?;
    drop(w);

    std::fs::rename(&partial_path, &out_path)
        .with_context(|| format!("rename {} → {}", partial_path.display(), out_path.display()))?;

    eprintln!(
        "Wrote {candles} candles in {batches} batches → {}",
        out_path.display()
    );
    Ok(())
}
