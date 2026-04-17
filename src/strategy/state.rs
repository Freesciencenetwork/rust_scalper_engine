use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FailedAcceptanceState {
    pub breakout_level: Option<f64>,
    pub active: bool,
}
