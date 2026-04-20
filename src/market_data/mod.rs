#![allow(clippy::pedantic, clippy::nursery)] // Large `prepare`/`snapshot` tables; pedantic churn on generated-style field wiring.

mod data;
mod prepare;
pub mod snapshot;

pub use data::{PreparedCandle, PreparedDataset};
pub use snapshot::{
    CandlestickPatternSnapshot, DirectionalSnapshot, IchimokuSnapshot, IndicatorSnapshot,
    MomentumSnapshot, OrderFlowSnapshot, PivotClassicSnapshot, PivotFibSnapshot, TrendSnapshot,
    VolatilitySnapshot, VolumeSnapshot,
};
