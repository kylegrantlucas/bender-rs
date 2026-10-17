//! Bent colour-burst / chroma decoder: hue rotation with per-band phase
//! wobble, saturation gain, and a misfiring colour killer.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::BurstBend;
use crate::stages::{BURST, stage_rng};

const WOBBLE_BAND: usize = 16;
const KILLER_BAND: usize = 8;

/// Rotates and scales chroma in place.
pub(crate) fn apply(s: &BurstBend, seed: i32, n: u64, planes: &mut YiqPlanes) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    let last_band = ((h - 1) / WOBBLE_BAND) as i64;
    let band_err = |b: i64| -> f32 {
        let b = b.clamp(0, last_band) as u64;
        stage_rng(seed, BURST, n).mix(b).mix(0).range(-1.0, 1.0)
    };
    planes
        .i
        .par_chunks_mut(w)
        .zip(planes.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, (is, qs))| {
            let killed = stage_rng(seed, BURST, n)
                .mix((r / KILLER_BAND) as u64)
                .mix(1)
                .chance(s.killer_rate);
            if killed {
                is.fill(0.0);
                qs.fill(0.0);
                return;
            }
            let err = if s.wobble != 0.0 {
                let t = (r as f32 + 0.5 - WOBBLE_BAND as f32 / 2.0) / WOBBLE_BAND as f32;
                let b0 = t.floor();
                let f = t - b0;
                let (e0, e1) = (band_err(b0 as i64), band_err(b0 as i64 + 1));
                e0 + (e1 - e0) * f
            } else {
                0.0
            };
            let (sin, cos) = (s.phase + s.wobble * err).to_radians().sin_cos();
            let (sc, ss) = (cos * s.saturation, sin * s.saturation);
            for (i, q) in is.iter_mut().zip(qs.iter_mut()) {
                let (oi, oq) = (*i, *q);
                *i = oi * sc - oq * ss;
                *q = oi * ss + oq * sc;
            }
        });
}
