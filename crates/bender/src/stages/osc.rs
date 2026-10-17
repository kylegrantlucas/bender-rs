//! Self-oscillating op-amp: a slanted, drifting sine pattern leaks into the
//! luma and/or chroma, stronger on brighter pixels when luma-coupled. Near
//! instability the amplifier also rings after edges at the same frequency.

use std::f32::consts::TAU;

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{OscTarget, Oscillation};

/// Pole radius of the edge resonator.
const RHO: f32 = 0.9;
/// Ringing gain. The spec's `0.5·(1-ρ)` gives a step overshoot of only ~0.11
/// at frequency 0.05 (peak of `ρ^m·sin((m+1)ω)/sin ω` ≈ 2.24), at the edge of
/// the required 0.1–0.4; doubling it to `1.0·(1-ρ)` centres it at ~0.22.
const RING_K: f32 = 1.0 - RHO;

/// Adds the oscillation and edge ringing in place.
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
    let ringing = s.ringing > 0.0;
    let b1 = 2.0 * RHO * (TAU * s.frequency).cos();
    let b2 = RHO * RHO;
    let rk = s.ringing * RING_K;
    planes
        .y
        .par_chunks_mut(w)
        .zip(planes.i.par_chunks_mut(w))
        .zip(planes.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((ys, is), qs))| {
            let row_phase = (r as f64 * f64::from(s.slant) + frame_term).rem_euclid(1.0) as f32;
            // Resonator state per component: (r[x-1], r[x-2], original c[x-1]).
            let mut st = [
                (0.0f32, 0.0f32, ys[0]),
                (0.0, 0.0, is[0]),
                (0.0, 0.0, qs[0]),
            ];
            let ring = |st: &mut (f32, f32, f32), c: f32| -> f32 {
                let v = b1 * st.0 - b2 * st.1 + (c - st.2);
                *st = (v, st.0, c);
                v
            };
            for x in 0..w {
                let (oy, oi, oq) = (ys[x], is[x], qs[x]);
                let phi = TAU * (x as f32 * s.frequency + row_phase);
                let a = s.amplitude * (1.0 - s.luma_coupling + s.luma_coupling * oy);
                let (sin, cos) = phi.sin_cos();
                if chroma {
                    is[x] += 0.6 * a * sin;
                    qs[x] += 0.6 * a * cos;
                    if ringing {
                        is[x] += rk * ring(&mut st[1], oi);
                        qs[x] += rk * ring(&mut st[2], oq);
                    }
                }
                if luma {
                    ys[x] += a * sin;
                    if ringing {
                        ys[x] += rk * ring(&mut st[0], oy);
                    }
                }
            }
        });
}
