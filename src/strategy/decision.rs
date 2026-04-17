use serde::{Deserialize, Serialize};

use crate::domain::VolatilityRegime;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalDecision {
    pub allowed: bool,
    pub reasons: Vec<String>,
    pub regime: Option<VolatilityRegime>,
    pub trigger_price: Option<f64>,
    pub atr: Option<f64>,
}
