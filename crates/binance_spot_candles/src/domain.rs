use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candle {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub close_time: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub buy_volume: Option<f64>,
    pub sell_volume: Option<f64>,
    pub delta: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolFilters {
    pub tick_size: f64,
    pub lot_step: f64,
}

impl Candle {
    pub fn inferred_delta(&self) -> Option<f64> {
        self.delta
            .or_else(|| match (self.buy_volume, self.sell_volume) {
                (Some(buy), Some(sell)) => Some(buy - sell),
                _ => None,
            })
    }
}
