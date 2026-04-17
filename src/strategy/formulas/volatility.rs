pub fn target_move_pct(target_atr_multiple: f64, atr: f64, entry_price: f64) -> f64 {
    (target_atr_multiple * atr) / entry_price.max(f64::EPSILON)
}
