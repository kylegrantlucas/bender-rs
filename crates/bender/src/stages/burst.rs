//! Bent colour-burst / chroma decoder: hue rotation with per-band phase
//! wobble, saturation gain, a misfiring colour killer, PAL-style Hanover
//! phase alternation and an encoder that drops every other burst.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{BurstBend, StageId};
use crate::stages::stage_rng;

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
        stage_rng(seed, StageId::Burst, n)
            .mix(b)
            .mix(0)
            .range(-1.0, 1.0)
    };
    planes
        .i
        .par_chunks_mut(w)
        .zip(planes.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, (is, qs))| {
            let killed = stage_rng(seed, StageId::Burst, n)
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
            let odd = r % 2 == 1;
            let mut angle = s.phase + s.wobble * err;
            if odd {
                angle *= 1.0 - 2.0 * s.hanover;
            }
            let (sin, cos) = angle.to_radians().sin_cos();
            let sat = if odd {
                s.saturation * (1.0 - s.alternate_loss)
            } else {
                s.saturation
            };
            let (sc, ss) = (cos * sat, sin * sat);
            for (i, q) in is.iter_mut().zip(qs.iter_mut()) {
                let (oi, oq) = (*i, *q);
                *i = oi * sc - oq * ss;
                *q = oi * ss + oq * sc;
            }
        });
}
