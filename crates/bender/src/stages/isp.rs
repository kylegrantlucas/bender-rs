//! Camera DSP with bent control loops: an auto-exposure loop that overshoots
//! and pumps, an auto-white-balance loop that swings colour casts, an
//! over-driven aperture correction that halos edges, and sensor noise that
//! grows with the exposure gain.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{CameraIsp, StageId};
use crate::stages::memory::{Continuity, IspLoop};
use crate::stages::stage_rng;

/// Applies the loop state entering frame `n`, updates it from this frame's
/// statistics, then sharpens and adds gain-scaled noise.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply(
    s: &CameraIsp,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
    buf: &mut Vec<f32>,
    memory: &mut Continuity<IspLoop>,
) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    memory.advance(n);
    let (before, after) = memory.split();
    let st = *before;

    // Gain / white balance, with per-row sums for the loop measurements.
    buf.clear();
    buf.resize(3 * h, 0.0);
    planes
        .y
        .par_chunks_mut(w)
        .zip(planes.i.par_chunks_mut(w))
        .zip(planes.q.par_chunks_mut(w))
        .zip(buf.par_chunks_mut(3))
        .for_each(|(((ys, is), qs), sums)| {
            let (mut sy, mut si, mut sq) = (0.0f64, 0.0f64, 0.0f64);
            for y in ys.iter_mut() {
                *y *= st.gain;
                sy += f64::from(*y);
            }
            for i in is.iter_mut() {
                *i -= st.wb_i;
                si += f64::from(*i);
            }
            for q in qs.iter_mut() {
                *q -= st.wb_q;
                sq += f64::from(*q);
            }
            sums[0] = sy as f32;
            sums[1] = si as f32;
            sums[2] = sq as f32;
        });
    let (mut ty, mut ti, mut tq) = (0.0f64, 0.0f64, 0.0f64);
    for sums in buf.as_chunks::<3>().0 {
        ty += f64::from(sums[0]);
        ti += f64::from(sums[1]);
        tq += f64::from(sums[2]);
    }
    let count = (w * h) as f64;
    let (m, mi, mq) = (
        (ty / count) as f32,
        (ti / count) as f32,
        (tq / count) as f32,
    );

    let exponent = 0.2 + 1.8 * s.hunt;
    let wb_k = 0.2 + 1.8 * s.white_balance;
    *after = IspLoop {
        gain: (st.gain * (0.45 / m.max(0.02)).powf(exponent)).clamp(0.25, 4.0),
        wb_i: st.wb_i + wb_k * mi,
        wb_q: st.wb_q + wb_k * mq,
    };

    let sharpen = s.sharpen;
    let noise = s.noise * 0.05 * st.gain;
    if sharpen == 0.0 && noise == 0.0 {
        return;
    }

    // Separable box blur of Y: horizontal pass into `buf`, vertical into `scratch.y`.
    let radius = s.sharpen_radius.max(1) as usize;
    if sharpen != 0.0 {
        buf.clear();
        buf.resize(w * h, 0.0);
        buf.par_chunks_mut(w)
            .zip(planes.y.par_chunks(w))
            .for_each(|(out, row)| {
                let inv = 1.0 / (2 * radius + 1) as f32;
                let at = |k: isize| row[k.clamp(0, w as isize - 1) as usize];
                let r = radius as isize;
                let mut sum: f32 = (-r..=r).map(at).sum();
                for (x, o) in out.iter_mut().enumerate() {
                    *o = sum * inv;
                    let x = x as isize;
                    sum += at(x + r + 1) - at(x - r);
                }
            });
        scratch.y.resize(w * h, 0.0);
        let hbuf: &[f32] = buf;
        scratch
            .y
            .par_chunks_mut(w)
            .enumerate()
            .for_each(|(r, out)| {
                let inv = 1.0 / (2 * radius + 1) as f32;
                out.fill(0.0);
                for k in r as isize - radius as isize..=(r + radius) as isize {
                    let src = k.clamp(0, h as isize - 1) as usize;
                    for (o, v) in out.iter_mut().zip(&hbuf[src * w..(src + 1) * w]) {
                        *o += v;
                    }
                }
                for o in out.iter_mut() {
                    *o *= inv;
                }
            });
    }

    let frame = stage_rng(seed, StageId::Isp, n);
    let blur: &[f32] = &scratch.y;
    planes.y.par_chunks_mut(w).enumerate().for_each(|(r, ys)| {
        if sharpen != 0.0 {
            for (y, b) in ys.iter_mut().zip(&blur[r * w..(r + 1) * w]) {
                *y += sharpen * (*y - b);
            }
        }
        if noise != 0.0 {
            let mut rng = frame.mix(r as u64);
            for y in ys.iter_mut() {
                *y += noise * rng.range(-1.0, 1.0);
            }
        }
    });
}
