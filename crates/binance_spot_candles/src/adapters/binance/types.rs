use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BinanceExchangeInfo {
    pub symbols: Vec<BinanceSymbol>,
}

#[derive(Debug, Deserialize)]
pub struct BinanceSymbol {
    pub symbol: String,
    pub filters: Vec<BinanceFilter>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "filterType")]
pub enum BinanceFilter {
    #[serde(rename = "PRICE_FILTER")]
    PriceFilter {
        #[serde(rename = "tickSize")]
        tick_size: String,
    },
    #[serde(rename = "LOT_SIZE")]
    LotSize {
        #[serde(rename = "stepSize")]
        step_size: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct BinanceKlineCsvRow {
    #[serde(rename = "open_time")]
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    #[serde(rename = "close_time")]
    pub close_time: i64,
    #[serde(default)]
    pub quote_asset_volume: Option<String>,
    #[serde(default)]
    pub number_of_trades: Option<u64>,
    #[serde(default)]
    pub taker_buy_base_volume: Option<String>,
    #[serde(default)]
    pub taker_buy_quote_volume: Option<String>,
    #[serde(default)]
    pub ignore: Option<String>,
}
