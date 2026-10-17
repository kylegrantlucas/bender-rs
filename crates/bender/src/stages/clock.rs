//! Bent ADC pixel clock: samples are held across several pixels, the sample
//! rate is stretched, and the clock occasionally stalls mid-row.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::ClockBend;
use crate::stages::{CLOCK, sample_nearest_wrap, stage_rng};

/// Resamples each row with a held/stretched/stalling clock, gathering into `scratch`.
pub(crate) fn apply(
    s: &ClockBend,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);
    let hold = s.sample_hold.max(1) as usize;
    let src_x = |x: usize| -> isize { (((x / hold) * hold) as f32 * s.stretch).floor() as isize };

    let src = &*planes;
    scratch
        .y
        .par_chunks_mut(w)
        .zip(scratch.i.par_chunks_mut(w))
        .zip(scratch.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yo, io), qo))| {
            let mut rowrng = stage_rng(seed, CLOCK, n).mix(r as u64);
            let (mut g_start, mut g_end) = (0usize, 0usize);
            if rowrng.chance(s.glitch_rate) {
                g_start = (rowrng.range(0.0, w as f32) as usize).min(w - 1);
                let len = (rowrng.range(0.1, 1.0) * w as f32) as usize;
                g_end = (g_start + len).min(w);
            }
            let stalled = src_x(g_start);
            let range = r * w..(r + 1) * w;
            let (ys, is, qs) = (&src.y[range.clone()], &src.i[range.clone()], &src.q[range]);
            for x in 0..w {
                let sx = if x >= g_start && x < g_end {
                    stalled
                } else {
                    src_x(x)
                };
                yo[x] = sample_nearest_wrap(ys, sx);
                io[x] = sample_nearest_wrap(is, sx);
                qo[x] = sample_nearest_wrap(qs, sx);
            }
        });
    std::mem::swap(planes, scratch);
}
