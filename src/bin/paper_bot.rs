#![allow(non_snake_case)] // Same package name as library crate (`binance_BTC`).
#![allow(clippy::multiple_crate_versions)] // Transitive duplicates; see `lib.rs`.

//! Minimal **paper** loop: pull Binance `15m` history, run [`DecisionMachine`], print advice and a toy portfolio.
//!
//! Not execution-grade (no exchange, no real stops).  
//! **Run:** `cargo run --bin paper_bot -- --help`

use std::time::Duration;

use anyhow::{Context, Result};
use binance_BTC::domain::SymbolFilters;
use binance_BTC::{DecisionMachine, MachineAction, MachineRequest, RuntimeState, StrategyConfig};
use binance_spot_candles::adapters::binance::{
    BinanceExchangeInfoAdapter, fetch_klines, http_client,
};
use binance_spot_candles::adapters::traits::SymbolMetadataAdapter;
use clap::Parser;
use reqwest::Client;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(
    name = "paper_bot",
    about = "Paper loop: Binance 15m klines + DecisionMachine (no real orders)"
)]
struct Cli {
    #[arg(long, default_value = "BTCUSDT")]
    symbol: String,
    #[arg(long, default_value = "15m")]
    interval: String,
    #[arg(long, default_value = "https://api.binance.com")]
    base_url: String,
    /// Klines per REST call (max 1000). Must cover `vol_baseline_lookback_bars`.
    #[arg(long, default_value_t = 1000)]
    kline_limit: u16,
    /// Starting paper USDT cash (sizing uses mark-to-market equity each step).
    #[arg(long, default_value_t = 100_000.0)]
    paper_equity_usdt: f64,
    /// Overrides `StrategyConfig::vol_baseline_lookback_bars` (default 960). Use **96** with `--kline-limit 96` for a tiny demo.
    #[arg(long, default_value_t = 960)]
    vol_baseline_lookback_bars: usize,
    /// **0** = single snapshot. **>0** = sleep seconds between pulls (e.g. **900** ≈ 15m).
    #[arg(long, default_value_t = 0)]
    watch_secs: u64,
}

#[derive(Debug, Default)]
struct PaperState {
    usdt: f64,
    btc: f64,
    avg_entry: f64,
    stop_price: Option<f64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = StrategyConfig::default();
    let min_lookback = config
        .vwma_lookback
        .max(config.runway_lookback)
        .max(if config.vp_enabled {
            config.vp_lookback_bars
        } else {
            1
        })
        .max(2);
    let vb = cli.vol_baseline_lookback_bars.max(min_lookback);
    config.vol_baseline_lookback_bars = vb;

    let machine = DecisionMachine::new(config);
    let client = http_client().context("HTTP client")?;
    let filters = fetch_symbol_filters(&client, &cli.base_url, &cli.symbol)
        .await
        .context("exchangeInfo")?;

    let mut paper = PaperState {
        usdt: cli.paper_equity_usdt,
        btc: 0.0,
        avg_entry: 0.0,
        stop_price: None,
    };

    loop {
        let candles = fetch_klines(
            &client,
            &cli.base_url,
            &cli.symbol,
            &cli.interval,
            cli.kline_limit,
            None,
            None,
        )
        .await
        .context("klines")?;

        let n = candles.len();
        let mark = candles.last().context("empty klines")?.close;
        let equity_for_sizing = paper.usdt + paper.btc * mark;

        let request = MachineRequest {
            candles_15m: candles,
            macro_events: Vec::new(),
            runtime_state: RuntimeState::default(),
            account_equity: Some(equity_for_sizing),
            symbol_filters: Some(filters.clone()),
            rustyfish_overlay: None,
            config_overrides: None,
        };

        let response = machine
            .evaluate(request)
            .with_context(|| format!("evaluate failed (need ≥{vb} closed 15m bars; got {n})"))?;

        let diag_close = response.diagnostics.latest_frame.candle.close;

        println!(
            "--- as_of={} | bars={} | action={:?} | allowed={} | cash={:.2} USDT | btc={:.6} | mark={:.2} | mtm_equity≈{:.2} | reasons {:?}",
            response.diagnostics.as_of,
            n,
            response.action,
            response.decision.allowed,
            paper.usdt,
            paper.btc,
            diag_close,
            paper.usdt + paper.btc * diag_close,
            response.decision.reasons
        );

        if paper.btc > 0.0
            && let Some(stop) = paper.stop_price
            && diag_close <= stop
        {
            let proceeds = paper.btc * diag_close;
            paper.usdt += proceeds;
            println!(
                "PAPER: simulated STOP {:.6} BTC @ {:.2} (diag close) — flat",
                paper.btc, diag_close
            );
            paper.btc = 0.0;
            paper.avg_entry = 0.0;
            paper.stop_price = None;
        }

        if matches!(response.action, MachineAction::ArmLongStop)
            && let Some(plan) = &response.plan
            && let Some(qty) = plan.qty_btc
            && qty > 0.0
            && paper.btc <= 0.0
        {
            let fill = plan.trigger_price;
            let cost = qty * fill;
            if cost <= paper.usdt {
                paper.usdt -= cost;
                paper.btc = qty;
                paper.avg_entry = fill;
                paper.stop_price = Some(plan.stop_price);
                println!(
                    "PAPER: simulated BUY {:.6} BTC @ {:.2} (trigger); stop {:.2} target {:.2}",
                    qty, fill, plan.stop_price, plan.target_price
                );
            } else {
                println!(
                    "PAPER: skip buy — need {:.2} USDT, have {:.2}",
                    cost, paper.usdt
                );
            }
        }

        if cli.watch_secs == 0 {
            break;
        }
        sleep(Duration::from_secs(cli.watch_secs)).await;
    }

    Ok(())
}

async fn fetch_symbol_filters(
    client: &Client,
    base_url: &str,
    symbol: &str,
) -> Result<SymbolFilters> {
    let url = format!(
        "{}/api/v3/exchangeInfo?symbol={}",
        base_url.trim_end_matches('/'),
        symbol
    );
    let body = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    BinanceExchangeInfoAdapter::parse_symbol_filters(&body, symbol).context("parse_symbol_filters")
}
