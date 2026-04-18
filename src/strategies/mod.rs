//! Pluggable decision strategies; each reads [`PreparedDataset`](crate::market_data::PreparedDataset).

pub mod default;

use crate::config::StrategyConfig;
use crate::domain::SystemMode;
use crate::market_data::PreparedDataset;
use crate::strategy::decision::SignalDecision;

/// Runtime strategy selection (library core, not HTTP-specific).
pub trait Strategy {
    fn id(&self) -> &'static str;
    fn set_system_mode(&mut self, mode: SystemMode);
    /// Replay failed-acceptance state from `fa_start` through `index` (inclusive).
    fn replay_failed_acceptance_window(
        &mut self,
        fa_start: usize,
        index: usize,
        dataset: &PreparedDataset,
    );
    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision;
}

impl Strategy for default::StrategyEngine {
    fn id(&self) -> &'static str {
        default::DEFAULT_STRATEGY_ID
    }

    fn set_system_mode(&mut self, mode: SystemMode) {
        self.system_mode = mode;
    }

    fn replay_failed_acceptance_window(
        &mut self,
        fa_start: usize,
        index: usize,
        dataset: &PreparedDataset,
    ) {
        for frame_index in fa_start..=index {
            self.update_failed_acceptance(frame_index, dataset);
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

pub fn strategy_engine_for(config: &StrategyConfig) -> anyhow::Result<Box<dyn Strategy>> {
    match config.strategy_id.as_str() {
        default::DEFAULT_STRATEGY_ID => Ok(Box::new(default::StrategyEngine::new(config.clone()))),
        other => anyhow::bail!("unknown strategy_id: {other}"),
    }
}
