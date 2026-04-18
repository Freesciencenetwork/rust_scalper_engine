//! CLI: pull Spot market data from Binance (klines + symbol filters).

use anyhow::{Context, Result};
use binance_BTC::adapters::binance::{
    BinanceExchangeInfoAdapter, fetch_klines, http_client,
};
use binance_BTC::adapters::traits::SymbolMetadataAdapter;
use binance_BTC::{MachineRequest, RuntimeState};
use clap::{Parser, Subcommand};

const DEFAULT_BASE: &str = "https://api.binance.com";

#[derive(Parser)]
#[command(name = "binance-fetch", version, about = "Fetch Binance Spot data for the decision machine")]
struct Cli {
    /// Binance Spot REST root (no trailing slash), e.g. https://testnet.binance.vision for spot testnet
    #[arg(long, default_value = DEFAULT_BASE)]
    base_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download klines and print a JSON `MachineRequest` body (ready for `POST /v1/evaluate`).
    Klines {
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "15m")]
        interval: String,
        /// Binance allows at most 1000 klines per request
        #[arg(long, default_value_t = 1000)]
        limit: u16,
        #[arg(long)]
        start_time: Option<i64>,
        #[arg(long)]
        end_time: Option<i64>,
    },
    /// Fetch `exchangeInfo` and print `symbol_filters` JSON for the symbol
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

            let request = MachineRequest {
                candles_15m: candles,
                macro_events: Vec::new(),
                runtime_state: RuntimeState::default(),
                account_equity: None,
                symbol_filters: None,
                rustyfish_overlay: None,
            };

            println!(
                "{}",
                serde_json::to_string_pretty(&request).context("serialize MachineRequest")?
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
