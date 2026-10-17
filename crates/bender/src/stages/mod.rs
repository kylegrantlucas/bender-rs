//! Individual bend stages, applied in signal-chain order by `effect::run_chain`.

pub(crate) mod address;
pub(crate) mod bleed;
pub(crate) mod burst;
pub(crate) mod clock;
pub(crate) mod data;
pub(crate) mod hold;
pub(crate) mod invert;
pub(crate) mod osc;
pub(crate) mod rail;
pub(crate) mod sync;

use crate::random::Rng;

pub(crate) const SYNC: u64 = 1;
pub(crate) const CLOCK: u64 = 2;
pub(crate) const DATA: u64 = 3;
pub(crate) const HOLD: u64 = 4;
pub(crate) const ADDRESS_A: u64 = 5;
pub(crate) const ADDRESS_B: u64 = 6;
// Invert, rail, osc and bleed are deterministic; their ids are reserved to keep numbering stable.
#[expect(dead_code, reason = "reserved stage id; stage draws no randomness")]
pub(crate) const INVERT: u64 = 7;
#[expect(dead_code, reason = "reserved stage id; stage draws no randomness")]
pub(crate) const RAIL: u64 = 8;
#[expect(dead_code, reason = "reserved stage id; stage draws no randomness")]
pub(crate) const OSC: u64 = 9;
pub(crate) const BURST: u64 = 10;
#[expect(dead_code, reason = "reserved stage id; stage draws no randomness")]
pub(crate) const BLEED: u64 = 11;

/// Per-stage, per-frame random stream: `Rng::new(seed as u32 as u64).mix(stage).mix(frame)`.
pub(crate) fn stage_rng(seed: i32, stage: u64, frame: u64) -> Rng {
    Rng::new(u64::from(seed as u32)).mix(stage).mix(frame)
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
