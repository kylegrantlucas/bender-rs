//! Starved or shorted sync separator: rows lose horizontal lock (skew, jitter,
//! tear bands) and the picture rolls vertically past the blanking interval.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::SyncBend;
use crate::stages::{SYNC, sample_linear_wrap, stage_rng};

/// Re-times every row horizontally and vertically, gathering into `scratch`.
pub(crate) fn apply(
    s: &SyncBend,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);
    let wf = w as f64;

    let mut frame = stage_rng(seed, SYNC, n);
    let (mut tear_start, mut tear_end, mut tear_shift) = (0usize, 0usize, 0.0f64);
    if frame.chance(s.tear_rate) {
        tear_start = frame.range(0.0, h as f32) as usize;
        let len = (frame.range(0.05, 0.3) * h as f32) as usize;
        tear_end = tear_start + len;
        tear_shift = f64::from(frame.range(-0.5, 0.5)) * wf;
    }

    let roll = ((n as f64 * f64::from(s.v_roll) * 0.05).rem_euclid(1.0) * h as f64) as usize;
    let blank_from = h - (h / 24).max(1);
    let rolling = s.v_roll != 0.0;

    let src = &*planes;
    scratch
        .y
        .par_chunks_mut(w)
        .zip(scratch.i.par_chunks_mut(w))
        .zip(scratch.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yo, io), qo))| {
            let sr = (r + roll) % h;
            if rolling && sr >= blank_from {
                yo.fill(0.0);
                io.fill(0.0);
                qo.fill(0.0);
                return;
            }
            let mut rowrng = stage_rng(seed, SYNC, n).mix(r as u64);
            let in_tear = r >= tear_start && r < tear_end;
            let mut shift = f64::from(s.h_skew) * wf * r as f64
                + f64::from(rowrng.range(-1.0, 1.0) * s.h_jitter) * wf;
            if in_tear {
                shift += tear_shift;
            }
            let shift = shift.rem_euclid(wf) as f32;
            let lift = if in_tear { 0.1 } else { 0.0 };
            let range = sr * w..(sr + 1) * w;
            let (ys, is, qs) = (&src.y[range.clone()], &src.i[range.clone()], &src.q[range]);
            for x in 0..w {
                let sx = x as f32 - shift;
                yo[x] = sample_linear_wrap(ys, sx) + lift;
                io[x] = sample_linear_wrap(is, sx);
                qo[x] = sample_linear_wrap(qs, sx);
            }
        });
    std::mem::swap(planes, scratch);
}
