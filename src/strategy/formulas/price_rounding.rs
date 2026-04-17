pub fn floor_to_step(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).floor() * step
}

pub fn round_up_to_step(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).ceil() * step
}

pub fn round_down_to_step(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).floor() * step
}

#[cfg(test)]
mod tests {
    use super::floor_to_step;

    #[test]
    fn floor_to_step_keeps_lot_rounding_stable() {
        assert!((floor_to_step(1.2345, 0.001) - 1.234).abs() < 1e-9);
    }
}
