use super::price_rounding::round_up_to_step;

pub fn buy_stop_trigger_price(signal_high: f64, tick_size: f64) -> f64 {
    round_up_to_step(signal_high + tick_size, tick_size)
}
