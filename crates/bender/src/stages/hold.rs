//! Glitched frame-store write-enable: bands, fields or whole frames of the
//! store miss their write and keep showing (optionally fading) content from
//! the previous frame.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{HoldMode, MemoryHold, StageId};
use crate::stages::memory::{Continuity, matching, store};
use crate::stages::stage_rng;

/// Replaces un-written rows with the previous frame's output.
pub(crate) fn apply(
    s: &MemoryHold,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    memory: &mut Continuity<Option<YiqPlanes>>,
) {
    memory.advance(n);
    let (before, after) = memory.split();
    let (w, h) = (planes.width, planes.height);
    if let Some(prev) = matching(before, w, h)
        && w > 0
    {
        let band_rows = s.band_height.max(1) as usize;
        let strobe_held = !n.is_multiple_of(s.strobe_period.max(1) as u64);
        let frame = stage_rng(seed, StageId::Hold, n);
        let field_held = [frame.mix(0).chance(s.rate), frame.mix(1).chance(s.rate)];
        let held = |r: usize| match s.mode {
            HoldMode::Bands => frame.mix((r / band_rows) as u64).chance(s.rate),
            HoldMode::Fields => field_held[r & 1],
            HoldMode::Strobe => strobe_held,
        };
        let d = s.decay;
        planes
            .y
            .par_chunks_mut(w)
            .zip(planes.i.par_chunks_mut(w))
            .zip(planes.q.par_chunks_mut(w))
            .enumerate()
            .for_each(|(r, ((ys, is), qs))| {
                if !held(r) {
                    return;
                }
                let range = r * w..(r + 1) * w;
                for (o, p) in ys.iter_mut().zip(&prev.y[range.clone()]) {
                    *o = p * (1.0 - d) + 0.5 * d;
                }
                for (o, p) in is.iter_mut().zip(&prev.i[range.clone()]) {
                    *o = p * (1.0 - d);
                }
                for (o, p) in qs.iter_mut().zip(&prev.q[range]) {
                    *o = p * (1.0 - d);
                }
            });
    }
    store(after, planes);
}
