//! Self-oscillating op-amp: a slanted, drifting sine pattern leaks into the
//! luma and/or chroma, stronger on brighter pixels when luma-coupled.

use std::f32::consts::TAU;

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{OscTarget, Oscillation};

/// Adds the oscillation in place.
pub(crate) fn apply(s: &Oscillation, n: u64, planes: &mut YiqPlanes) {
    let w = planes.width;
    if w == 0 || planes.height == 0 {
        return;
    }
    let (luma, chroma) = match s.target {
        OscTarget::Luma => (true, false),
        OscTarget::Chroma => (false, true),
        OscTarget::Both => (true, true),
    };
    let frame_term = n as f64 * f64::from(s.drift);
    planes
        .y
        .par_chunks_mut(w)
        .zip(planes.i.par_chunks_mut(w))
        .zip(planes.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((ys, is), qs))| {
            let row_phase = (r as f64 * f64::from(s.slant) + frame_term).rem_euclid(1.0) as f32;
            for x in 0..w {
                let phi = TAU * (x as f32 * s.frequency + row_phase);
                let a = s.amplitude * (1.0 - s.luma_coupling + s.luma_coupling * ys[x]);
                let (sin, cos) = phi.sin_cos();
                if chroma {
                    is[x] += 0.6 * a * sin;
                    qs[x] += 0.6 * a * cos;
                }
                if luma {
                    ys[x] += a * sin;
                }
            }
        });
}
