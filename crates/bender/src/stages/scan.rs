//! Scan processor: a Rutt/Etra-style raster manipulator plus a Paik/Abe
//! wobbulator. The picture's brightness drives the vertical deflection, so
//! every Nth scanline is lifted into a relief of lines, and an audio-rate
//! sine on the deflection bends the whole raster.

use std::f64::consts::TAU;

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::ScanProcessor;

/// Output rows per parallel band.
const BAND: usize = 16;

/// Redraws the picture as brightness-displaced scanlines over a dimmed copy
/// of the original.
pub(crate) fn apply(s: &ScanProcessor, n: u64, planes: &mut YiqPlanes, scratch: &mut YiqPlanes) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);
    let src: &YiqPlanes = planes;
    let spacing = s.line_spacing.max(1) as usize;
    let thickness = i64::from(s.thickness.max(1));
    let mix = s.mix;
    let base = 1.0 - mix;
    let lift = s.displace * 0.25 * h as f32;
    let wobble = s.wobble * 0.05 * h as f32;
    let freq = f64::from(s.wobble_freq);
    let frame_off = n as f64 * (w as f64) * (h as f64);

    let yd = |row: &[f32], mean: f32, sr: usize, x: usize| -> f32 {
        let phase = (freq * ((sr * w + x) as f64 + frame_off)).rem_euclid(1.0);
        sr as f32 - lift * (row[x] - mean) + wobble * (TAU * phase).sin() as f32
    };

    scratch
        .y
        .par_chunks_mut(BAND * w)
        .zip(scratch.i.par_chunks_mut(BAND * w))
        .zip(scratch.q.par_chunks_mut(BAND * w))
        .enumerate()
        .for_each(|(band, ((oy, oi), oq))| {
            let r0 = band * BAND;
            let rows = oy.len() / w;
            let off = r0 * w;
            for k in 0..oy.len() {
                oy[k] = src.y[off + k] * base;
                oi[k] = src.i[off + k] * base;
                oq[k] = src.q[off + k] * base;
            }
            let (lo_band, hi_band) = (r0 as i64, (r0 + rows) as i64 - 1);
            for sr in (0..h).step_by(spacing) {
                let row = &src.y[sr * w..(sr + 1) * w];
                let mean = if s.ac_coupled {
                    row.iter().sum::<f32>() / w as f32
                } else {
                    0.0
                };
                let mut prev = 0.0;
                for x in 0..w {
                    let cur = yd(row, mean, sr, x);
                    let (a, b) = if x == 0 {
                        (cur, cur)
                    } else {
                        (cur.min(prev), cur.max(prev))
                    };
                    prev = cur;
                    let top = (a.round() as i64).max(lo_band);
                    let bottom = (b.round() as i64 + thickness - 1).min(hi_band);
                    if top > bottom {
                        continue;
                    }
                    let p = sr * w + x;
                    let (ly, li, lq) = (src.y[p] * mix, src.i[p] * mix, src.q[p] * mix);
                    for r in top..=bottom {
                        let o = (r as usize - r0) * w + x;
                        let g = off + o;
                        oy[o] = ly + src.y[g] * base;
                        oi[o] = li + src.i[g] * base;
                        oq[o] = lq + src.q[g] * base;
                    }
                }
            }
        });
    std::mem::swap(planes, scratch);
}
