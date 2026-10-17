//! A camera re-shooting its own monitor: the previous output returns zoomed,
//! rotated, offset and hue-shifted, softened by the lens/CRT and keyed into
//! the live picture.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::FeedbackLoop;
use crate::stages::memory::{Continuity, matching, store};

/// Bilinear sample of one plane at pixel coordinates; texels outside are 0.
fn sample(plane: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let texel = |tx: f32, ty: f32| -> f32 {
        if tx < 0.0 || ty < 0.0 || tx >= w as f32 || ty >= h as f32 {
            0.0
        } else {
            plane[ty as usize * w + tx as usize]
        }
    };
    let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
    let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
    top * (1.0 - fy) + bottom * fy
}

/// Sample with the blur cross: lerp of the centre tap toward its 4-neighbour mean.
fn blurred(plane: &[f32], w: usize, h: usize, x: f32, y: f32, blur: f32) -> f32 {
    let c = sample(plane, w, h, x, y);
    if blur <= 0.0 {
        return c;
    }
    let m = (sample(plane, w, h, x - 1.0, y)
        + sample(plane, w, h, x + 1.0, y)
        + sample(plane, w, h, x, y - 1.0)
        + sample(plane, w, h, x, y + 1.0))
        * 0.25;
    c + (m - c) * blur
}

/// Mixes the transformed previous output of this stage into the live picture.
pub(crate) fn apply(
    s: &FeedbackLoop,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
    memory: &mut Continuity<Option<YiqPlanes>>,
) {
    memory.advance(n);
    let (before, after) = memory.split();
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    let Some(prev) = matching(before, w, h) else {
        store(after, planes);
        return;
    };
    scratch.resize(w, h);

    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let (ox, oy) = (s.offset_x * w as f32, s.offset_y * h as f32);
    let inv_zoom = 1.0 / s.zoom.max(1e-3);
    // R(-rotate)
    let (sr, cr) = (-s.rotate).to_radians().sin_cos();
    let (sh, ch) = s.hue_shift.to_radians().sin_cos();
    let cur = &*planes;

    scratch
        .y
        .par_chunks_mut(w)
        .zip(scratch.i.par_chunks_mut(w))
        .zip(scratch.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((ys, is), qs))| {
            let dy = r as f32 - cy - oy;
            for x in 0..w {
                let dx = x as f32 - cx - ox;
                let qx = cx + (cr * dx - sr * dy) * inv_zoom;
                let qy = cy + (sr * dx + cr * dy) * inv_zoom;
                let fy = (blurred(&prev.y, w, h, qx, qy, s.blur) * s.gain).clamp(0.0, 1.0);
                let fi0 = blurred(&prev.i, w, h, qx, qy, s.blur);
                let fq0 = blurred(&prev.q, w, h, qx, qy, s.blur);
                let fi = fi0 * ch - fq0 * sh;
                let fq = fi0 * sh + fq0 * ch;

                let p = r * w + x;
                let (y, i, q) = (cur.y[p], cur.i[p], cur.q[p]);
                let key = if s.key > 0.0 {
                    ((s.key - y) / 0.05 + 0.5).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let a = s.amount * key;
                ys[x] = y + (fy - y) * a;
                is[x] = i + (fi - i) * a;
                qs[x] = q + (fq - q) * a;
            }
        });

    std::mem::swap(planes, scratch);
    store(after, planes);
}
