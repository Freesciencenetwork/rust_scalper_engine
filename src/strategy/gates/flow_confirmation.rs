use crate::strategy::data::PreparedCandle;

pub fn passes(frame: &PreparedCandle) -> bool {
    frame.cvd_ema3_slope.unwrap_or(0.0) > 0.0
}
