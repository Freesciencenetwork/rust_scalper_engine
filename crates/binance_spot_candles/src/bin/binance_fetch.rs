//! CLI: pull Spot market data from Binance (klines + symbol filters).

use anyhow::{Context, Result};
use binance_spot_candles::adapters::binance::{
    BinanceExchangeInfoAdapter, fetch_klines, http_client,
};
use binance_spot_candles::adapters::traits::SymbolMetadataAdapter;
use clap::{Parser, Subcommand};

const DEFAULT_BASE: &str = "https://api.binance.com";

#[derive(Parser)]
#[command(name = "binance-fetch", version, about = "Fetch Binance Spot klines / exchangeInfo")]
struct Cli {
    #[arg(long, default_value = DEFAULT_BASE)]
    base_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download klines and print JSON shaped like a `MachineRequest` body (for `POST /v1/evaluate`).
    Klines {
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "15m")]
        interval: String,
        #[arg(long, default_value_t = 1000)]
        limit: u16,
        #[arg(long)]
        start_time: Option<i64>,
        #[arg(long)]
        end_time: Option<i64>,
    },
    SymbolFilters {
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = http_client().context("HTTP client")?;

    match cli.command {
        Command::Klines {
            symbol,
            interval,
            limit,
            start_time,
            end_time,
        } => {
            if limit == 0 || limit > 1000 {
                anyhow::bail!("--limit must be between 1 and 1000 (Binance API cap)");
            }
            let candles = fetch_klines(
                &client,
                &cli.base_url,
                &symbol,
                &interval,
                limit,
                start_time,
                end_time,
            )
            .await?;

            let candles_value =
                serde_json::to_value(&candles).context("serialize candles_15m")?;
            let request = serde_json::json!({
                "candles_15m": candles_value,
                "macro_events": [],
                "runtime_state": {
                    "realized_net_r_today": 0.0,
                    "halt_new_entries_flag": 0
                },
                "account_equity": serde_json::Value::Null,
                "symbol_filters": serde_json::Value::Null,
                "rustyfish_overlay": serde_json::Value::Null,
            });

            println!(
                "{}",
                serde_json::to_string_pretty(&request).context("serialize request JSON")?
            );
        }
        Command::SymbolFilters { symbol } => {
            let url = format!(
                "{}/api/v3/exchangeInfo?symbol={}",
                cli.base_url.trim_end_matches('/'),
                symbol
            );
            let body = client
                .get(url)
                .send()
                .await
                .context("exchangeInfo request failed")?
                .error_for_status()
                .context("exchangeInfo returned error status")?
                .text()
                .await
                .context("read exchangeInfo body")?;

            let filters =
                BinanceExchangeInfoAdapter::parse_symbol_filters(&body, &symbol).with_context(
                    || format!("parse symbol filters for {symbol}"),
                )?;

            println!(
                "{}",
                serde_json::to_string_pretty(&filters).context("serialize SymbolFilters")?
            );
        }
    }

    Ok(())
}
