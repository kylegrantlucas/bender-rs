//! NTSC composite link: the picture is resampled to 4·fsc, encoded into a
//! real composite signal (band-limited I/Q quadrature-modulated onto the
//! subcarrier), sent down a cable that adds a reflection and snow, and decoded
//! by a cheap monitor (notch or 1H comb separation, peaking, a possibly
//! detuned subcarrier oscillator) before being resampled back to the frame.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{CompositeLink, Decoder, StageId};
use crate::stages::stage_rng;

/// Samples per active line at 4·fsc (192 subcarrier cycles).
const S: usize = 768;
/// Ghost delay in samples.
const GHOST_DELAY: usize = 31;

#[inline]
fn at(row: &[f32], k: isize) -> f32 {
    row[k.clamp(0, S as isize - 1) as usize]
}

/// Linear sample of `row` at fractional position `x`, clamped to the row.
#[inline]
fn sample(row: &[f32], x: f32) -> f32 {
    let last = row.len() - 1;
    let x = x.clamp(0.0, last as f32);
    let x0 = x as usize;
    let x1 = (x0 + 1).min(last);
    row[x0] + (row[x1] - row[x0]) * (x - x0 as f32)
}

/// Centred box mean of `2r+1` taps over clamped indices, as a sliding sum.
fn box_mean(src: &[f32], r: isize, dst: &mut [f32]) {
    let n = (2 * r + 1) as f32;
    let mut sum: f32 = (-r..=r).map(|j| at(src, j)).sum();
    for (k, out) in dst.iter_mut().enumerate() {
        let k = k as isize;
        *out = sum / n;
        sum += at(src, k + r + 1) - at(src, k - r);
    }
}

/// Encoder subcarrier phase of sample `k` on row `r`.
#[inline]
fn phase(k: usize, base: f32) -> f32 {
    (k % 4) as f32 * FRAC_PI_2 + base
}

/// Runs the picture through an NTSC composite encode, cable and decode.
pub(crate) fn apply(
    s: &CompositeLink,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    buf: &mut Vec<f32>,
) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    let len = h * S;
    buf.clear();
    buf.resize(4 * len, 0.0);
    let (sig, rest) = buf.split_at_mut(len);
    let (yd, rest) = rest.split_at_mut(len);
    let (id, qd) = rest.split_at_mut(len);

    let to_src = w as f32 / S as f32;
    // 1. Upsample to 4·fsc.
    yd.par_chunks_mut(S)
        .zip(id.par_chunks_mut(S))
        .zip(qd.par_chunks_mut(S))
        .zip(
            planes
                .y
                .par_chunks(w)
                .zip(planes.i.par_chunks(w))
                .zip(planes.q.par_chunks(w)),
        )
        .for_each(|(((yo, io), qo), ((ys, is), qs))| {
            for k in 0..S {
                let x = (k as f32 + 0.5) * to_src - 0.5;
                yo[k] = sample(ys, x);
                io[k] = sample(is, x);
                qo[k] = sample(qs, x);
            }
        });

    let crawl = if s.dot_crawl {
        PI * (n % 2) as f32
    } else {
        0.0
    };
    let row_base = |r: usize| 33f32.to_radians() + PI * (r % 2) as f32 + crawl;
    let frame_rng = stage_rng(seed, StageId::Composite, n);

    // 2. Encode, add ghost and noise.
    sig.par_chunks_mut(S)
        .zip(yd.par_chunks(S))
        .zip(id.par_chunks(S).zip(qd.par_chunks(S)))
        .enumerate()
        .for_each(|(r, ((so, ys), (is, qs)))| {
            let base = row_base(r);
            let mut bi = [0.0f32; S];
            let mut bq = [0.0f32; S];
            box_mean(is, 2, &mut bi);
            box_mean(qs, 6, &mut bq);
            for k in 0..S {
                let (sn, cs) = phase(k, base).sin_cos();
                so[k] = ys[k] + bi[k] * cs + bq[k] * sn;
            }
            if s.ghost > 0.0 {
                let g = s.ghost * 0.5;
                for k in (GHOST_DELAY..S).rev() {
                    so[k] += g * so[k - GHOST_DELAY];
                }
            }
            if s.noise > 0.0 {
                let mut rng = frame_rng.mix(r as u64);
                let a = s.noise * 0.25;
                for v in so.iter_mut() {
                    *v += a * rng.range(-1.0, 1.0);
                }
            }
        });

    // 3–4. Separate, trap, peak, demodulate.
    let sig: &[f32] = sig;
    let comb = s.decoder == Decoder::Comb && h > 1;
    yd.par_chunks_mut(S)
        .zip(id.par_chunks_mut(S))
        .zip(qd.par_chunks_mut(S))
        .enumerate()
        .for_each(|(r, ((yo, io), qo))| {
            let sr = &sig[r * S..(r + 1) * S];
            let mut c = [0.0f32; S];
            if comb {
                let p = if r == 0 { 1 } else { r - 1 };
                let sp = &sig[p * S..(p + 1) * S];
                for k in 0..S {
                    c[k] = (sr[k] - sp[k]) * 0.5;
                }
            } else {
                for k in 0..S {
                    let ki = k as isize;
                    let ylp = (0.5 * at(sr, ki - 2)
                        + at(sr, ki - 1)
                        + sr[k]
                        + at(sr, ki + 1)
                        + 0.5 * at(sr, ki + 2))
                        * 0.25;
                    c[k] = sr[k] - ylp;
                }
            }
            for k in 0..S {
                yo[k] = sr[k] - s.trap * c[k];
            }
            if s.peaking > 0.0 {
                io.copy_from_slice(yo);
                let mut m = [0.0f32; S];
                box_mean(io, 3, &mut m);
                for k in 0..S {
                    yo[k] = io[k] + s.peaking * (io[k] - m[k]);
                }
            }
            // Products 2·C·cos/sin of the decoder phase, then sliding means.
            let base = row_base(r);
            let mut pc = [0.0f32; S];
            let mut ps = [0.0f32; S];
            for k in 0..S {
                let err = TAU * s.detune * (4.0 * k as f32 / S as f32 + 0.37 * r as f32);
                let (sn, cs) = (phase(k, base) + err).sin_cos();
                pc[k] = 2.0 * c[k] * cs;
                ps[k] = 2.0 * c[k] * sn;
            }
            window_mean(&pc, 8, io);
            window_mean(&ps, 12, qo);
        });

    // 5. Downsample back to the frame.
    let to_sig = S as f32 / w as f32;
    planes
        .y
        .par_chunks_mut(w)
        .zip(planes.i.par_chunks_mut(w))
        .zip(planes.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yo, io), qo))| {
            let rows = r * S..(r + 1) * S;
            let (ys, is, qs) = (&yd[rows.clone()], &id[rows.clone()], &qd[rows]);
            for x in 0..w {
                let k = (x as f32 + 0.5) * to_sig - 0.5;
                yo[x] = sample(ys, k);
                io[x] = sample(is, k);
                qo[x] = sample(qs, k);
            }
        });
}

/// Mean of `taps` samples over the window `k - taps/2 .. k + taps/2` (clamped).
fn window_mean(src: &[f32], taps: isize, dst: &mut [f32]) {
    let lo = taps / 2;
    let hi = taps - lo - 1;
    let n = taps as f32;
    let mut sum: f32 = (-lo..=hi).map(|j| at(src, j)).sum();
    for (k, out) in dst.iter_mut().enumerate() {
        let k = k as isize;
        *out = sum / n;
        sum += at(src, k + hi + 1) - at(src, k - lo);
    }
}
