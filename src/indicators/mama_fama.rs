//! Mesa adaptive moving average (MAMA) and following adaptive MA (FAMA).
//! Ported from QuantConnect `MesaAdaptiveMovingAverage.cs` (price = (H+L)/2).

use std::collections::VecDeque;

use crate::domain::Candle;

const SMALL: f64 = 0.0962;
const LARGE: f64 = 0.5769;

#[derive(Clone, Debug, PartialEq)]
pub struct MamaFamaBar {
    pub mama: f64,
    pub fama: f64,
}

fn rad2deg() -> f64 {
    180.0 / std::f64::consts::PI
}

/// `fast_limit` / `slow_limit` default `0.5` / `0.05` (Lean defaults).
pub fn mama_fama_series(
    candles: &[Candle],
    fast_limit: f64,
    slow_limit: f64,
) -> Vec<Option<MamaFamaBar>> {
    let n = candles.len();
    let mut out = vec![None; n];
    let mut ph: VecDeque<f64> = VecDeque::with_capacity(13);
    let mut smooth_hist: VecDeque<f64> = VecDeque::with_capacity(6);
    let mut detrend_hist: VecDeque<f64> = VecDeque::with_capacity(6);
    let mut inphase_hist: VecDeque<f64> = VecDeque::with_capacity(6);
    let mut quad_hist: VecDeque<f64> = VecDeque::with_capacity(6);

    let mut prev_period = 0.0_f64;
    let mut prev_in_phase2 = 0.0_f64;
    let mut prev_quadrature2 = 0.0_f64;
    let mut prev_real = 0.0_f64;
    let mut prev_imaginary = 0.0_f64;
    let mut prev_smooth_period = 0.0_f64;
    let mut prev_phase = 0.0_f64;
    let mut prev_mama = 0.0_f64;
    let mut prev_fama = 0.0_f64;

    for i in 0..n {
        let mid = (candles[i].high + candles[i].low) / 2.0;
        ph.push_back(mid);
        if ph.len() > 13 {
            ph.pop_front();
        }
        if ph.len() < 13 {
            continue;
        }
        let p = |k: usize| *ph.iter().rev().nth(k).unwrap();
        let adjusted_period = 0.075 * prev_period + 0.54;
        let smooth = (4.0 * p(0) + 3.0 * p(1) + 2.0 * p(2) + p(3)) / 10.0;

        let detrender = if smooth_hist.len() < 6 {
            0.0
        } else {
            let s = |k: usize| *smooth_hist.iter().rev().nth(k).unwrap();
            (SMALL * smooth + LARGE * s(1) - LARGE * s(3) - SMALL * s(5)) * adjusted_period
        };

        let (quadrature1, in_phase1) = if detrend_hist.len() < 6 {
            (0.0, 0.0)
        } else {
            let d = |k: usize| *detrend_hist.iter().rev().nth(k).unwrap();
            let q1 =
                (SMALL * detrender + LARGE * d(1) - LARGE * d(3) - SMALL * d(5)) * adjusted_period;
            let i1 = *detrend_hist.iter().rev().nth(2).unwrap();
            (q1, i1)
        };

        let (adjusted_in_phase, adjusted_quadrature) =
            if inphase_hist.len() < 6 || quad_hist.len() < 6 {
                (0.0, 0.0)
            } else {
                let ip = |k: usize| *inphase_hist.iter().rev().nth(k).unwrap();
                let qu = |k: usize| *quad_hist.iter().rev().nth(k).unwrap();
                let ai = (SMALL * in_phase1 + LARGE * ip(1) - LARGE * ip(3) - SMALL * ip(5))
                    * adjusted_period;
                let aq = (SMALL * quadrature1 + LARGE * qu(1) - LARGE * qu(3) - SMALL * qu(5))
                    * adjusted_period;
                (ai, aq)
            };

        let mut in_phase2 = in_phase1 - adjusted_quadrature;
        let mut quadrature2 = quadrature1 + adjusted_in_phase;
        in_phase2 = 0.2 * in_phase2 + 0.8 * prev_in_phase2;
        quadrature2 = 0.2 * quadrature2 + 0.8 * prev_quadrature2;

        let mut real = in_phase2 * prev_in_phase2 + quadrature2 * prev_quadrature2;
        let mut imaginary = in_phase2 * prev_quadrature2 - quadrature2 * prev_in_phase2;
        real = 0.2 * real + 0.8 * prev_real;
        imaginary = 0.2 * imaginary + 0.8 * prev_imaginary;

        let mut period = 0.0_f64;
        if imaginary.abs() > f64::EPSILON && real.abs() > f64::EPSILON {
            let angle = (imaginary / real).atan() * rad2deg();
            period = if angle > 0.0 { 360.0 / angle } else { 0.0 };
        }

        if period > 1.5 * prev_period {
            period = 1.5 * prev_period;
        }
        if period < 0.67 * prev_period {
            period = 0.67 * prev_period;
        }
        period = period.clamp(6.0, 50.0);

        period = 0.2 * period + 0.8 * prev_period;
        let smooth_period = 0.33 * period + 0.67 * prev_smooth_period;

        let mut phase = 0.0_f64;
        if in_phase1.abs() > f64::EPSILON {
            phase = (quadrature1 / in_phase1).atan() * rad2deg();
        }

        let mut delta_phase = prev_phase - phase;
        if delta_phase < 1.0 {
            delta_phase = 1.0;
        }

        let mut alpha = fast_limit / delta_phase;
        if alpha < slow_limit {
            alpha = slow_limit;
        }

        let mama = alpha * p(0) + (1.0 - alpha) * prev_mama;
        let fama = 0.5 * alpha * mama + (1.0 - 0.5 * alpha) * prev_fama;

        smooth_hist.push_back(smooth);
        if smooth_hist.len() > 6 {
            smooth_hist.pop_front();
        }
        detrend_hist.push_back(detrender);
        if detrend_hist.len() > 6 {
            detrend_hist.pop_front();
        }
        inphase_hist.push_back(in_phase1);
        if inphase_hist.len() > 6 {
            inphase_hist.pop_front();
        }
        quad_hist.push_back(quadrature1);
        if quad_hist.len() > 6 {
            quad_hist.pop_front();
        }

        prev_in_phase2 = in_phase2;
        prev_quadrature2 = quadrature2;
        prev_real = real;
        prev_imaginary = imaginary;
        prev_period = period;
        prev_smooth_period = smooth_period;
        prev_phase = phase;
        prev_mama = mama;
        prev_fama = fama;

        if i + 1 >= 33 {
            out[i] = Some(MamaFamaBar { mama, fama });
        }
    }
    out
}
