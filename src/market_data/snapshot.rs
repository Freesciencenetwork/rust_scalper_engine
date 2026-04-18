use serde::{Deserialize, Serialize};

/// Extra TA indicators computed every bar (default strategy may ignore these).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MomentumSnapshot {
    pub rsi_14: Option<f64>,
    pub macd_line: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_hist: Option<f64>,
    pub stoch_k: Option<f64>,
    pub stoch_d: Option<f64>,
    pub stoch_rsi_k: Option<f64>,
    pub stoch_rsi_d: Option<f64>,
    pub cci_20: Option<f64>,
    pub williams_r_14: Option<f64>,
    pub roc_10: Option<f64>,
    pub mfi_14: Option<f64>,
    pub ultosc_7_14_28: Option<f64>,
    pub tsi_25_13: Option<f64>,
    pub awesome_oscillator_5_34: Option<f64>,
    pub ppo_line: Option<f64>,
    pub ppo_signal: Option<f64>,
    pub ppo_hist: Option<f64>,
    pub kst: Option<f64>,
    pub elder_bull: Option<f64>,
    pub elder_bear: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IchimokuSnapshot {
    pub tenkan_9: Option<f64>,
    pub kijun_26: Option<f64>,
    pub senkou_a_26: Option<f64>,
    pub senkou_b_52: Option<f64>,
    pub chikou_close_shifted: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrendSnapshot {
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_200: Option<f64>,
    pub ema_20: Option<f64>,
    pub wma_20: Option<f64>,
    pub hull_9: Option<f64>,
    pub vwap_session: Option<f64>,
    pub vwap_upper_1sd: Option<f64>,
    pub vwap_lower_1sd: Option<f64>,
    pub vwap_upper_2sd: Option<f64>,
    pub vwap_lower_2sd: Option<f64>,
    pub dema_20: Option<f64>,
    pub tema_20: Option<f64>,
    pub mcginley_14: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PivotClassicSnapshot {
    pub pivot_p: Option<f64>,
    pub pivot_r1: Option<f64>,
    pub pivot_r2: Option<f64>,
    pub pivot_r3: Option<f64>,
    pub pivot_s1: Option<f64>,
    pub pivot_s2: Option<f64>,
    pub pivot_s3: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PivotFibSnapshot {
    pub pivot_p: Option<f64>,
    pub pivot_r1: Option<f64>,
    pub pivot_r2: Option<f64>,
    pub pivot_r3: Option<f64>,
    pub pivot_s1: Option<f64>,
    pub pivot_s2: Option<f64>,
    pub pivot_s3: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VolatilitySnapshot {
    pub bb_middle_20: Option<f64>,
    pub bb_upper_20: Option<f64>,
    pub bb_lower_20: Option<f64>,
    pub bb_pct_b_20: Option<f64>,
    pub bb_bandwidth_20: Option<f64>,
    pub keltner_middle_20: Option<f64>,
    pub keltner_upper_20: Option<f64>,
    pub keltner_lower_20: Option<f64>,
    pub donchian_upper_20: Option<f64>,
    pub donchian_lower_20: Option<f64>,
    pub donchian_mid_20: Option<f64>,
    pub supertrend_10_3: Option<f64>,
    pub supertrend_long: Option<bool>,
    pub mass_index_25: Option<f64>,
    pub pivot_classic: PivotClassicSnapshot,
    pub pivot_fib: PivotFibSnapshot,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DirectionalSnapshot {
    pub adx_14: Option<f64>,
    pub di_plus: Option<f64>,
    pub di_minus: Option<f64>,
    pub aroon_up_25: Option<f64>,
    pub aroon_down_25: Option<f64>,
    pub psar: Option<f64>,
    pub psar_trend_long: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VolumeSnapshot {
    pub obv: Option<f64>,
    pub ad_line: Option<f64>,
    pub cmf_20: Option<f64>,
    pub volume_sma_20: Option<f64>,
    pub volume_ema_20: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndicatorSnapshot {
    pub momentum: MomentumSnapshot,
    pub trend: TrendSnapshot,
    pub ichimoku: IchimokuSnapshot,
    pub volatility: VolatilitySnapshot,
    pub directional: DirectionalSnapshot,
    pub volume: VolumeSnapshot,
}
