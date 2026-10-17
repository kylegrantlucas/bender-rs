//! Starved or shorted sync separator: rows lose horizontal lock (skew, hunting
//! jitter, tear bands that relock), the blanking interval shows where shifted
//! lines wrap, the back-porch clamp can sample picture, and the picture rolls
//! vertically past the blanking interval.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{StageId, SyncBend};
use crate::stages::{sample_linear_wrap, stage_rng};

/// Line period relative to active width: NTSC blanking is 10.9 µs of 63.6 µs.
const LINE_PERIOD: f32 = 1.208;

/// Re-times every row horizontally and vertically, gathering into `scratch`.
pub(crate) fn apply(
    s: &SyncBend,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
    buf: &mut Vec<f32>,
) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);
    let wf = w as f64;

    let mut frame = stage_rng(seed, StageId::Sync, n);
    let (mut tear_start, mut tear_end, mut tear_shift) = (0usize, 0usize, 0.0f64);
    if frame.chance(s.tear_rate) {
        tear_start = frame.range(0.0, h as f32) as usize;
        let len = (frame.range(0.05, 0.3) * h as f32) as usize;
        tear_end = tear_start + len;
        tear_shift = f64::from(frame.range(-0.5, 0.5)) * wf;
    }
    let tear_len = (tear_end - tear_start).max(1) as f64;

    // Hunting H-PLL: row jitter is an AR(1) process over the per-row draws.
    let a = s.jitter_smooth.clamp(0.0, 0.99);
    let innov = (1.0 - a * a).sqrt();
    buf.clear();
    buf.resize(h, 0.0);
    let mut prev = 0.0f32;
    for (r, j) in buf.iter_mut().enumerate() {
        let u = stage_rng(seed, StageId::Sync, n)
            .mix(r as u64)
            .range(-1.0, 1.0);
        prev = if r == 0 { u } else { a * prev + innov * u };
        *j = prev;
    }
    let jitter = &buf[..];

    let roll = ((n as f64 * f64::from(s.v_roll) * 0.05).rem_euclid(1.0) * h as f64) as usize;
    let blank_from = h - (h / 24).max(1);
    let rolling = s.v_roll != 0.0;
    let period = if s.blanking {
        f64::from(w as f32 * LINE_PERIOD)
    } else {
        wf
    };
    let lp = period as f32;
    let last = (w - 1) as f32;
    let clamp_cols = (w / 20).max(1);

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
            let in_tear = r >= tear_start && r < tear_end;
            let mut shift =
                f64::from(s.h_skew) * wf * r as f64 + f64::from(jitter[r] * s.h_jitter) * wf;
            if in_tear {
                shift += tear_shift * (-3.0 * (r - tear_start) as f64 / tear_len).exp();
            }
            let shift = shift.rem_euclid(period) as f32;
            let lift = if in_tear { 0.1 } else { 0.0 };
            let range = sr * w..(sr + 1) * w;
            let (ys, is, qs) = (&src.y[range.clone()], &src.i[range.clone()], &src.q[range]);
            if s.blanking {
                for x in 0..w {
                    let sx = (x as f32 - shift).rem_euclid(lp);
                    let (yv, iv, qv) = if sx <= last {
                        (
                            sample_linear_wrap(ys, sx),
                            sample_linear_wrap(is, sx),
                            sample_linear_wrap(qs, sx),
                        )
                    } else if sx < w as f32 {
                        let k = 1.0 - (sx - last);
                        (ys[w - 1] * k, is[w - 1] * k, qs[w - 1] * k)
                    } else if sx > lp - 1.0 {
                        let k = sx - (lp - 1.0);
                        (ys[0] * k, is[0] * k, qs[0] * k)
                    } else {
                        (0.0, 0.0, 0.0)
                    };
                    yo[x] = yv + lift;
                    io[x] = iv;
                    qo[x] = qv;
                }
            } else {
                for x in 0..w {
                    let sx = x as f32 - shift;
                    yo[x] = sample_linear_wrap(ys, sx) + lift;
                    io[x] = sample_linear_wrap(is, sx);
                    qo[x] = sample_linear_wrap(qs, sx);
                }
            }
            if s.clamp_error != 0.0 {
                let m = yo[..clamp_cols].iter().sum::<f32>() / clamp_cols as f32;
                let off = s.clamp_error * m;
                yo.iter_mut().for_each(|v| *v -= off);
            }
        });
    std::mem::swap(planes, scratch);
}
