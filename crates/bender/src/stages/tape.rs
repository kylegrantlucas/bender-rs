//! Worn VHS playback: FM luma loses bandwidth, colour-under chroma smears and
//! jitters, the capstan and drum wobble the timebase, the heads switch with a
//! skew at the bottom of the frame, a mistracking band drifts up the picture,
//! oxide dropouts streak white and over-deviated highlights trail black comets.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::random::Rng;
use crate::settings::{StageId, TapePlayback};
use crate::stages::stage_rng;

/// Sub-stream tags so independent per-row draws never share a sequence.
const CHROMA: u64 = 1;
const HEAD: u64 = 2;
const TRACK: u64 = 3;
const DROPOUT: u64 = 4;

fn row_rng(seed: i32, n: u64, r: usize, tag: u64) -> Rng {
    stage_rng(seed, StageId::Tape, n).mix(r as u64).mix(tag)
}

/// Zero-phase one-pole low-pass: forward then backward pass, in place.
fn smooth(row: &mut [f32], k: f32) {
    let Some(&first) = row.first() else { return };
    let mut acc = first;
    for v in row.iter_mut() {
        acc = k * acc + (1.0 - k) * *v;
        *v = acc;
    }
    let mut acc = row[row.len() - 1];
    for v in row.iter_mut().rev() {
        acc = k * acc + (1.0 - k) * *v;
        *v = acc;
    }
}

fn pole(tc: f32) -> f32 {
    (-1.0 / tc.max(1e-3)).exp()
}

/// Linear sample with black outside the row.
fn sample_black(row: &[f32], sx: f32) -> f32 {
    let x0 = sx.floor();
    let t = sx - x0;
    let at = |x: f32| {
        if x >= 0.0 && (x as usize) < row.len() {
            row[x as usize]
        } else {
            0.0
        }
    };
    at(x0) * (1.0 - t) + at(x0 + 1.0) * t
}

/// Plays the picture back through a worn VHS transport, gathering into `scratch`.
pub(crate) fn apply(
    s: &TapePlayback,
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
    let wf = w as f32;
    let hf = h as f32;

    // 1–2. Luma softening, chroma smear, hue jitter and chroma noise.
    let luma_k = (s.luma_bw > 0.0).then(|| pole(s.luma_bw * 0.004 * wf));
    let chroma_k = pole(0.015 * wf * (0.3 + s.chroma_noise));
    let hue_max = s.chroma_noise * 20f32.to_radians();
    let noise_amp = s.chroma_noise * 0.05;
    planes
        .y
        .par_chunks_mut(w)
        .zip(planes.i.par_chunks_mut(w))
        .zip(planes.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yr, ir), qr))| {
            if let Some(k) = luma_k {
                smooth(yr, k);
            }
            smooth(ir, chroma_k);
            smooth(qr, chroma_k);
            let mut rng = row_rng(seed, n, r, CHROMA);
            let (sn, cs) = (hue_max * rng.range(-1.0, 1.0)).sin_cos();
            let (mut a, mut b) = (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
            for x in 0..w {
                if x > 0 && x % 8 == 0 {
                    a = b;
                    b = rng.range(-1.0, 1.0);
                }
                let v = a + (b - a) * (x % 8) as f32 / 8.0;
                let (i, q) = (ir[x], qr[x]);
                ir[x] = i * cs - q * sn + noise_amp * v;
                qr[x] = i * sn + q * cs + noise_amp * v;
            }
        });

    // 3–4, 6. Per-row shifts: timebase AR(1), flagging, head switch, tracking.
    let mut frame = stage_rng(seed, StageId::Tape, n);
    let head_factor = 0.6 + 0.4 * frame.next_f32();
    let head_rows = (h / 40).max(2);
    let head_start = h.saturating_sub(head_rows);
    let seedphase = stage_rng(seed, StageId::Tape, 0).next_f32();
    let centre = (1.0 - (n as f32 * 0.004 + seedphase).rem_euclid(1.0)) * 1.3 * hf - 0.15 * hf;
    let half = 0.03 * hf;
    let a = 0.9f32;
    let innov = (1.0 - a * a).sqrt();
    buf.clear();
    buf.resize(h, 0.0);
    let mut j = 0.0f32;
    for (r, shift) in buf.iter_mut().enumerate() {
        let u = stage_rng(seed, StageId::Tape, n)
            .mix(r as u64)
            .range(-1.0, 1.0);
        j = if r == 0 { u } else { a * j + innov * u };
        let rf = r as f32;
        let mut sh = j * s.jitter * 0.005 * wf + s.flagging * 0.05 * wf * (-rf / (0.04 * hf)).exp();
        if r >= head_start {
            sh += s.head_switch * 0.08 * wf * head_factor * (r - head_start + 1) as f32
                / head_rows as f32;
        }
        let dist = (rf - centre).abs();
        if s.tracking > 0.0 && dist < half {
            sh += s.tracking * 0.02 * wf * row_rng(seed, n, r, TRACK).range(-1.0, 1.0);
        }
        *shift = sh;
    }

    // 5–6. Gather with black outside, head-switch noise and tracking streaks.
    scratch.resize(w, h);
    let src = &*planes;
    let shifts = &buf[..];
    scratch
        .y
        .par_chunks_mut(w)
        .zip(scratch.i.par_chunks_mut(w))
        .zip(scratch.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yo, io), qo))| {
            let range = r * w..(r + 1) * w;
            let (ys, is, qs) = (&src.y[range.clone()], &src.i[range.clone()], &src.q[range]);
            let shift = shifts[r];
            for x in 0..w {
                let sx = x as f32 - shift;
                yo[x] = sample_black(ys, sx);
                io[x] = sample_black(is, sx);
                qo[x] = sample_black(qs, sx);
            }
            if r >= head_start && s.head_switch > 0.0 {
                let mut rng = row_rng(seed, n, r, HEAD);
                for v in yo.iter_mut() {
                    *v += s.head_switch * 0.15 * rng.range(-1.0, 1.0);
                }
            }
            let dist = (r as f32 - centre).abs();
            if dist < half {
                let p = s.tracking * (1.0 - dist / half);
                let mut rng = row_rng(seed, n, r, TRACK).mix(1);
                if rng.chance(p) {
                    let x0 = (rng.next_f32() * wf) as usize;
                    let len = (rng.range(0.05, 0.5) * wf) as usize;
                    let end = (x0 + len).min(w);
                    for x in x0..end {
                        yo[x] = 0.7 + 0.3 * rng.next_f32();
                        io[x] = 0.0;
                        qo[x] = 0.0;
                    }
                }
            }
        });
    std::mem::swap(planes, scratch);

    // 7. Dropouts, serial.
    let count = (s.dropout * 30.0 * frame.next_f32() + 0.5).floor() as usize;
    for d in 0..count {
        let mut rng = row_rng(seed, n, d, DROPOUT).mix(u64::MAX);
        let r = (rng.next_f32() * hf) as usize % h;
        let x0 = (rng.next_f32() * wf) as usize % w;
        let len = rng.range(2.0, (0.15 * wf).max(2.0)) as usize;
        let end = (x0 + len).min(w);
        let row = r * w;
        if s.doc && r > 0 {
            let above = row - w;
            planes.y.copy_within(above + x0..above + end, row + x0);
            planes.i.copy_within(above + x0..above + end, row + x0);
            planes.q.copy_within(above + x0..above + end, row + x0);
        } else if !s.doc {
            planes.y[row + x0..row + end].fill(0.95);
            planes.i[row + x0..row + end].fill(0.0);
            planes.q[row + x0..row + end].fill(0.0);
        }
    }

    // 8. Comets: highlights over-deviate and trail black.
    let comet_len = (s.comet * 0.03 * wf) as usize;
    if comet_len > 0 {
        planes.y.par_chunks_mut(w).for_each(|yr| {
            let mut cnt = 0usize;
            for v in yr.iter_mut() {
                if cnt > 0 {
                    *v = 0.05;
                    cnt -= 1;
                } else if *v > 0.92 {
                    cnt = comet_len;
                }
            }
        });
    }
}
