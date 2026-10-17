//! Glitched frame-store write-enable: bands of the frame store miss their
//! write and keep showing (optionally fading) content from the previous frame.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::MemoryHold;
use crate::stages::{HOLD, stage_rng};

/// Frame-store contents carried between frames of one stream.
#[derive(Default)]
pub(crate) struct HoldHistory {
    /// Output of the frame preceding `last_frame`.
    ram_prev: Option<YiqPlanes>,
    /// Output of `last_frame`.
    ram_last: Option<YiqPlanes>,
    last_frame: Option<u64>,
}

/// Replaces randomly un-written bands with the previous frame's output.
pub(crate) fn apply(
    s: &MemoryHold,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    history: &mut HoldHistory,
) {
    match history.last_frame {
        Some(last) if last == n => {}
        Some(last) if n > 0 && last == n - 1 => {
            std::mem::swap(&mut history.ram_prev, &mut history.ram_last);
        }
        _ => history.ram_prev = None,
    }

    let w = planes.width;
    if let Some(prev) = history.ram_prev.as_ref()
        && prev.width == w
        && prev.height == planes.height
        && w > 0
    {
        let band_len = s.band_height.max(1) as usize * w;
        let d = s.decay;
        planes
            .y
            .par_chunks_mut(band_len)
            .zip(planes.i.par_chunks_mut(band_len))
            .zip(planes.q.par_chunks_mut(band_len))
            .enumerate()
            .for_each(|(band, ((ys, is), qs))| {
                if !stage_rng(seed, HOLD, n).mix(band as u64).chance(s.rate) {
                    return;
                }
                let start = band * band_len;
                let range = start..start + ys.len();
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

    history
        .ram_last
        .get_or_insert_with(YiqPlanes::default)
        .copy_from(planes);
    history.last_frame = Some(n);
}
