//! Narrow, misaligned chroma path: the chroma low-pass smears colour to the
//! right of edges and a bent chroma delay line slips it out of register with luma.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::ChromaBleed;

/// Shifts and low-passes I and Q along each row (gathering into `scratch`); luma is untouched.
///
/// Chroma outside the active line is blanking (zero), so shifted-in and
/// filter-onset regions fade towards grey instead of wrapping.
pub(crate) fn apply(s: &ChromaBleed, planes: &mut YiqPlanes, scratch: &mut YiqPlanes) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 || (s.smear == 0.0 && s.shift == 0.0) {
        return;
    }
    scratch.resize(w, h);
    let shift = s.shift * w as f32;
    // Causal one-pole low-pass with a time constant of `smear` widths; k = 0 passes through.
    let k = if s.smear > 0.0 {
        (-1.0 / (s.smear * w as f32)).exp()
    } else {
        0.0
    };
    let last = (w - 1) as f32;
    let filter = |src: &[f32], dst: &mut [f32]| {
        let mut acc = 0.0f32;
        for (x, out) in dst.iter_mut().enumerate() {
            let sx = x as f32 - shift;
            let v = if (0.0..=last).contains(&sx) {
                let x0 = sx as usize;
                let x1 = (x0 + 1).min(w - 1);
                src[x0] + (src[x1] - src[x0]) * (sx - x0 as f32)
            } else {
                0.0
            };
            acc = v + (acc - v) * k;
            *out = acc;
        }
    };
    scratch
        .i
        .par_chunks_mut(w)
        .zip(scratch.q.par_chunks_mut(w))
        .zip(planes.i.par_chunks(w).zip(planes.q.par_chunks(w)))
        .for_each(|((io, qo), (is, qs))| {
            filter(is, io);
            filter(qs, qo);
        });
    std::mem::swap(&mut planes.i, &mut scratch.i);
    std::mem::swap(&mut planes.q, &mut scratch.q);
}
