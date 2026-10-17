//! Individual bend stages, dispatched in [`crate::BendEffect::order`] by `effect::run_chain`.

pub(crate) mod address;
pub(crate) mod bleed;
pub(crate) mod burst;
pub(crate) mod clock;
pub(crate) mod colorize;
pub(crate) mod composite;
pub(crate) mod counter;
pub(crate) mod data;
pub(crate) mod dct;
pub(crate) mod feedback;
pub(crate) mod hold;
pub(crate) mod invert;
pub(crate) mod isp;
pub(crate) mod memory;
pub(crate) mod mosh;
pub(crate) mod osc;
pub(crate) mod rail;
pub(crate) mod retina;
pub(crate) mod scan;
pub(crate) mod sensor;
pub(crate) mod sync;
pub(crate) mod tape;

use crate::random::Rng;
use crate::settings::StageId;

/// Per-stage, per-frame random stream: `Rng::new(seed as u32 as u64).mix(stage id).mix(frame)`.
pub(crate) fn stage_rng(seed: i32, stage: StageId, frame: u64) -> Rng {
    Rng::new(u64::from(seed as u32))
        .mix(stage.rng_id())
        .mix(frame)
}

/// Linearly interpolated sample at fractional `x`, wrapping both taps horizontally.
pub(crate) fn sample_linear_wrap(row: &[f32], x: f32) -> f32 {
    let w = row.len() as isize;
    if w == 0 {
        return 0.0;
    }
    let x0 = x.floor();
    let t = x - x0;
    let i0 = (x0 as isize).rem_euclid(w) as usize;
    let i1 = (x0 as isize + 1).rem_euclid(w) as usize;
    row[i0] + (row[i1] - row[i0]) * t
}

/// Nearest sample at integer `x`, wrapping horizontally.
pub(crate) fn sample_nearest_wrap(row: &[f32], x: isize) -> f32 {
    let w = row.len() as isize;
    if w == 0 {
        return 0.0;
    }
    row[x.rem_euclid(w) as usize]
}
