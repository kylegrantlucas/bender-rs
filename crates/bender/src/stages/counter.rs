//! Bent frame-store address counters: a wrong line pitch shears the picture,
//! early column/row resets tile it, and a creeping start address scrolls it.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::AddressCounter;

/// Reads each output pixel from the bent linear address, gathering into `scratch`.
pub(crate) fn apply(s: &AddressCounter, n: u64, planes: &mut YiqPlanes, scratch: &mut YiqPlanes) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);
    let line = ((s.line_length * w as f32).round() as usize).clamp(1, w);
    let frame = ((s.frame_length * h as f32).round() as usize).clamp(1, h);
    let total = (w * h) as i64;
    let start = (n as f64 * f64::from(s.scroll) * 0.05 * total as f64).round() as i64;
    let pitch = i64::from(s.pitch);
    let src = &*planes;
    scratch
        .y
        .par_chunks_mut(w)
        .zip(scratch.i.par_chunks_mut(w))
        .zip(scratch.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(y, ((oy, oi), oq))| {
            let row = (y % frame) as i64;
            let base = row * w as i64 + row * pitch + start;
            for x in 0..w {
                let col = (x % line) as i64;
                let a = (base + col).rem_euclid(total) as usize;
                oy[x] = src.y[a];
                oi[x] = src.i[a];
                oq[x] = src.q[a];
            }
        });
    std::mem::swap(planes, scratch);
}
