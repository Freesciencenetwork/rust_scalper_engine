mod evaluation;

use super::state::FailedAcceptanceState;
use crate::config::StrategyConfig;
use crate::domain::SystemMode;

#[derive(Clone, Debug)]
pub struct StrategyEngine {
    pub config: StrategyConfig,
    pub system_mode: SystemMode,
    pub(crate) failed_acceptance: FailedAcceptanceState,
}

impl StrategyEngine {
    pub fn new(config: StrategyConfig) -> Self {
        Self {
            config,
            system_mode: SystemMode::Active,
            failed_acceptance: Default::default(),
        }
    }
}
