use crate::market_data::PreparedCandle;

pub fn passes(frame: &PreparedCandle) -> bool {
    matches!(frame.vwma, Some(vwma) if frame.candle.close > vwma)
}
