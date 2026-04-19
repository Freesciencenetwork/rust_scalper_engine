//! Pluggable decision strategies; each reads [`crate::market_data::PreparedDataset`].

pub mod bb_mean_reversion;
pub mod default;
pub mod donchian_breakout;
pub mod ichimoku_trend;
pub mod macd_trend;
pub mod rsi_pullback;
pub mod safety;
pub mod stoch_crossover;
pub mod supertrend_adx;
pub mod ttm_squeeze_fire;

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

use bb_mean_reversion::BbMeanReversionEngine;
use default::gates::update_failed_acceptance;
use donchian_breakout::DonchianBreakoutEngine;
use ichimoku_trend::IchimokuTrendEngine;
use macd_trend::MacdTrendEngine;
use rsi_pullback::RsiPullbackEngine;
use stoch_crossover::StochCrossoverEngine;
use supertrend_adx::SupertrendAdxEngine;
use ttm_squeeze_fire::TtmSqueezeFireEngine;

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

impl Strategy for MacdTrendEngine {
    fn id(&self) -> &'static str {
        macd_trend::MACD_TREND_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

impl Strategy for RsiPullbackEngine {
    fn id(&self) -> &'static str {
        rsi_pullback::RSI_PULLBACK_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

impl Strategy for SupertrendAdxEngine {
    fn id(&self) -> &'static str {
        supertrend_adx::SUPERTREND_ADX_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

impl Strategy for BbMeanReversionEngine {
    fn id(&self) -> &'static str {
        bb_mean_reversion::BB_MEAN_REVERSION_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

impl Strategy for StochCrossoverEngine {
    fn id(&self) -> &'static str {
        stoch_crossover::STOCH_CROSSOVER_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

impl Strategy for IchimokuTrendEngine {
    fn id(&self) -> &'static str {
        ichimoku_trend::ICHIMOKU_TREND_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

impl Strategy for TtmSqueezeFireEngine {
    fn id(&self) -> &'static str {
        ttm_squeeze_fire::TTM_SQUEEZE_FIRE_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

impl Strategy for DonchianBreakoutEngine {
    fn id(&self) -> &'static str {
        donchian_breakout::DONCHIAN_BREAKOUT_STRATEGY_ID
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
            update_failed_acceptance(
                &mut self.failed_acceptance,
                frame_index,
                dataset,
                &self.config,
            );
        }
    }

    fn decide(&self, index: usize, dataset: &PreparedDataset) -> SignalDecision {
        self.evaluate_signal(index, dataset)
    }
}

pub fn strategy_engine_for(config: &StrategyConfig) -> anyhow::Result<Box<dyn Strategy>> {
    match config.strategy_id.as_str() {
        default::DEFAULT_STRATEGY_ID => Ok(Box::new(default::StrategyEngine::new(config.clone()))),
        macd_trend::MACD_TREND_STRATEGY_ID => Ok(Box::new(MacdTrendEngine::new(config.clone()))),
        rsi_pullback::RSI_PULLBACK_STRATEGY_ID => {
            Ok(Box::new(RsiPullbackEngine::new(config.clone())))
        }
        supertrend_adx::SUPERTREND_ADX_STRATEGY_ID => {
            Ok(Box::new(SupertrendAdxEngine::new(config.clone())))
        }
        bb_mean_reversion::BB_MEAN_REVERSION_STRATEGY_ID => {
            Ok(Box::new(BbMeanReversionEngine::new(config.clone())))
        }
        stoch_crossover::STOCH_CROSSOVER_STRATEGY_ID => {
            Ok(Box::new(StochCrossoverEngine::new(config.clone())))
        }
        ichimoku_trend::ICHIMOKU_TREND_STRATEGY_ID => {
            Ok(Box::new(IchimokuTrendEngine::new(config.clone())))
        }
        ttm_squeeze_fire::TTM_SQUEEZE_FIRE_STRATEGY_ID => {
            Ok(Box::new(TtmSqueezeFireEngine::new(config.clone())))
        }
        donchian_breakout::DONCHIAN_BREAKOUT_STRATEGY_ID => {
            Ok(Box::new(DonchianBreakoutEngine::new(config.clone())))
        }
        other => anyhow::bail!("unknown strategy_id: {other}"),
    }
}

#[cfg(test)]
mod strategy_id_tests {
    use super::*;

    #[test]
    fn strategy_engine_for_accepts_indicator_strategies() {
        for id in [
            macd_trend::MACD_TREND_STRATEGY_ID,
            rsi_pullback::RSI_PULLBACK_STRATEGY_ID,
            supertrend_adx::SUPERTREND_ADX_STRATEGY_ID,
            bb_mean_reversion::BB_MEAN_REVERSION_STRATEGY_ID,
            stoch_crossover::STOCH_CROSSOVER_STRATEGY_ID,
            ichimoku_trend::ICHIMOKU_TREND_STRATEGY_ID,
            ttm_squeeze_fire::TTM_SQUEEZE_FIRE_STRATEGY_ID,
            donchian_breakout::DONCHIAN_BREAKOUT_STRATEGY_ID,
        ] {
            let config = StrategyConfig {
                strategy_id: id.to_string(),
                ..Default::default()
            };
            let engine = strategy_engine_for(&config).unwrap_or_else(|err| panic!("{id}: {err}"));
            assert_eq!(engine.id(), id);
        }
    }
}
