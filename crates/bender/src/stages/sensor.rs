//! A bent CCD/CMOS imager: rolling-shutter skew and readout slip against the
//! previous frame, mains flicker under a rolling exposure, a colour-filter
//! array demosaiced with the wrong phase, vertical smear and blooming from
//! highlights, and fixed-pattern column gain with per-frame row noise.

use std::f32::consts::TAU;

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::random::Rng;
use crate::settings::{BayerPhase, SensorFault, StageId};
use crate::stages::memory::{Continuity, matching, store};
use crate::stages::stage_rng;

/// Highlight threshold above which charge smears and blooms.
const KNEE: f32 = 0.85;

/// RGGB colour of a site: 0 = R, 1 = G, 2 = B.
fn cfa(x: usize, y: usize) -> usize {
    match (x & 1, y & 1) {
        (0, 0) => 0,
        (1, 1) => 2,
        _ => 1,
    }
}

/// Runs the picture through the bent image sensor. `memory` holds the
/// previous frame's input to this stage.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply(
    s: &SensorFault,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
    buf: &mut Vec<f32>,
    memory: &mut Continuity<Option<YiqPlanes>>,
) {
    let (w, h) = (planes.width, planes.height);
    memory.advance(n);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);

    // 1–3. Temporal mix with the previous input; remember this input.
    {
        let (before, after) = memory.split();
        let prev = matching(before, w, h);
        let split = if s.slip != 0.0 {
            (((n as f64 * f64::from(s.slip) * 0.05).rem_euclid(1.0) * h as f64) as usize).min(h)
        } else {
            h
        };
        let denom = (h - 1).max(1) as f32;
        let src = &*planes;
        scratch
            .y
            .par_chunks_mut(w)
            .zip(scratch.i.par_chunks_mut(w))
            .zip(scratch.q.par_chunks_mut(w))
            .enumerate()
            .for_each(|(r, ((ys, is), qs))| {
                let range = r * w..(r + 1) * w;
                let (cy, ci, cq) = (
                    &src.y[range.clone()],
                    &src.i[range.clone()],
                    &src.q[range.clone()],
                );
                let Some(p) = prev else {
                    ys.copy_from_slice(cy);
                    is.copy_from_slice(ci);
                    qs.copy_from_slice(cq);
                    return;
                };
                let wp = if r >= split {
                    1.0
                } else {
                    s.skew * (1.0 - r as f32 / denom)
                };
                let (py, pi, pq) = (&p.y[range.clone()], &p.i[range.clone()], &p.q[range]);
                for x in 0..w {
                    ys[x] = cy[x] + (py[x] - cy[x]) * wp;
                    is[x] = ci[x] + (pi[x] - ci[x]) * wp;
                    qs[x] = cq[x] + (pq[x] - cq[x]) * wp;
                }
            });
        store(after, planes);
    }
    std::mem::swap(planes, scratch);

    // 4. Flicker.
    if s.flicker != 0.0 {
        let phase = 0.67 * (n % 300) as f32;
        planes.y.par_chunks_mut(w).enumerate().for_each(|(r, ys)| {
            let g = 1.0 + s.flicker * 0.3 * (TAU * (1.67 * r as f32 / h as f32 + phase)).sin();
            for v in ys {
                *v *= g;
            }
        });
    }

    // 5. Bayer mosaic and mis-phased demosaic.
    if s.bayer != BayerPhase::Off {
        let (dx, dy) = match s.bayer {
            BayerPhase::Horizontal => (1, 0),
            BayerPhase::Vertical => (0, 1),
            _ => (1, 1),
        };
        buf.clear();
        buf.resize(w * h, 0.0);
        {
            let src = &*planes;
            buf.par_chunks_mut(w).enumerate().for_each(|(r, row)| {
                let base = r * w;
                for (x, o) in row.iter_mut().enumerate() {
                    let (y, i, q) = (src.y[base + x], src.i[base + x], src.q[base + x]);
                    *o = match cfa(x, r) {
                        0 => y + 0.956 * i + 0.619 * q,
                        1 => y - 0.272 * i - 0.647 * q,
                        _ => y - 1.106 * i + 1.703 * q,
                    };
                }
            });
        }
        let mosaic = &*buf;
        scratch
            .y
            .par_chunks_mut(w)
            .zip(scratch.i.par_chunks_mut(w))
            .zip(scratch.q.par_chunks_mut(w))
            .enumerate()
            .for_each(|(r, ((ys, is), qs))| {
                for x in 0..w {
                    let mut rgb = [0.0f32; 3];
                    let own = cfa(x + dx, r + dy);
                    for (c, out) in rgb.iter_mut().enumerate() {
                        if c == own {
                            *out = mosaic[r * w + x];
                            continue;
                        }
                        let (mut sum, mut cnt) = (0.0f32, 0u32);
                        for oy in -1i64..=1 {
                            let sy = (r as i64 + oy).clamp(0, h as i64 - 1) as usize;
                            for ox in -1i64..=1 {
                                let sx = (x as i64 + ox).clamp(0, w as i64 - 1) as usize;
                                if cfa(sx + dx, sy + dy) == c {
                                    sum += mosaic[sy * w + sx];
                                    cnt += 1;
                                }
                            }
                        }
                        if cnt > 0 {
                            *out = sum / cnt as f32;
                        }
                    }
                    let [red, g, b] = rgb;
                    ys[x] = 0.299 * red + 0.587 * g + 0.114 * b;
                    is[x] = 0.5959 * red - 0.2746 * g - 0.3213 * b;
                    qs[x] = 0.2115 * red - 0.5227 * g + 0.3112 * b;
                }
            });
        std::mem::swap(planes, scratch);
    }

    // 6. Smear: white stripes down highlight columns.
    if s.smear != 0.0 {
        buf.clear();
        buf.resize(w, 0.0);
        for row in planes.y.chunks(w) {
            for (acc, &y) in buf.iter_mut().zip(row) {
                *acc += (y - KNEE).max(0.0);
            }
        }
        for v in buf.iter_mut() {
            *v = (s.smear * 12.0 * *v / h as f32).min(1.0);
        }
        let add = &*buf;
        planes
            .y
            .par_chunks_mut(w)
            .zip(planes.i.par_chunks_mut(w))
            .zip(planes.q.par_chunks_mut(w))
            .for_each(|((ys, is), qs)| {
                for x in 0..w {
                    let a = add[x];
                    let k = 1.0 / (1.0 + 4.0 * a);
                    ys[x] += a;
                    is[x] *= k;
                    qs[x] *= k;
                }
            });
    }

    // 7. Bloom: highlights leak up and down their column.
    if s.bloom != 0.0 {
        let k = (-1.0 / (s.bloom * 0.1 * h as f32).max(1e-3)).exp();
        buf.clear();
        buf.resize(w * h + w, 0.0);
        let (down, up) = buf.split_at_mut(w * h);
        let mut prev_row: Option<usize> = None;
        for r in 0..h {
            let base = r * w;
            for x in 0..w {
                let e = (planes.y[base + x] - KNEE).max(0.0);
                let carried = prev_row.map_or(0.0, |p| down[p + x] * k);
                down[base + x] = e.max(carried);
            }
            prev_row = Some(base);
        }
        for r in (0..h).rev() {
            let base = r * w;
            let ys = &mut planes.y[base..base + w];
            for x in 0..w {
                let e = (ys[x] - KNEE).max(0.0);
                up[x] = e.max(up[x] * k);
                ys[x] += s.bloom * down[base + x].max(up[x]);
            }
        }
    }

    // 8. Fixed-pattern noise: column gain fixed in time, row offset per frame.
    if s.fpn != 0.0 {
        let col = Rng::new(seed as u32 as u64)
            .mix(StageId::Sensor.rng_id())
            .mix(u64::MAX);
        let frame = stage_rng(seed, StageId::Sensor, n);
        planes.y.par_chunks_mut(w).enumerate().for_each(|(r, ys)| {
            let off = s.fpn * 0.02 * frame.mix(r as u64).range(-1.0, 1.0);
            for (c, v) in ys.iter_mut().enumerate() {
                let g = 1.0 + s.fpn * 0.1 * col.mix(c as u64).range(-1.0, 1.0);
                *v = *v * g + off;
            }
        });
    }
}
