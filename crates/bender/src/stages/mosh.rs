//! I-frame-drop datamosh: a block-motion decoder whose keyframes went
//! missing. Motion estimated on the live picture drags the stale previous
//! output around, with only part of the residual applied on top.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{Datamosh, StageId};
use crate::stages::memory::{Continuity, MoshFrames};
use crate::stages::stage_rng;

const BLOCK: usize = 16;

/// Predicts each 16×16 block from the previous output moved by the block's
/// (gained) motion vector, plus a fraction of the residual.
pub(crate) fn apply(
    s: &Datamosh,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
    memory: &mut Continuity<Option<MoshFrames>>,
) {
    let (w, h) = (planes.width, planes.height);
    memory.advance(n);
    let (before, after) = memory.split();
    if w == 0 || h == 0 {
        return;
    }
    let prev = before.as_ref().filter(|f| {
        f.input.width == w && f.input.height == h && f.output.width == w && f.output.height == h
    });
    let keyframe = stage_rng(seed, StageId::Mosh, n).chance(s.keyframe_rate);
    let frames = after.get_or_insert_with(MoshFrames::default);
    let Some(prev) = prev.filter(|_| !keyframe) else {
        frames.input.copy_from(planes);
        frames.output.copy_from(planes);
        return;
    };

    let search = s.search.max(0);
    let st = (search / 8).max(1) as usize;
    let gain = s.vector_gain;
    let residual = s.residual;
    let cur: &YiqPlanes = planes;
    let (wi, hi) = (w as isize, h as isize);
    let idx = |x: isize, y: isize| -> usize {
        (y.clamp(0, hi - 1) as usize) * w + x.clamp(0, wi - 1) as usize
    };

    scratch
        .y
        .par_chunks_mut(BLOCK * w)
        .zip(scratch.i.par_chunks_mut(BLOCK * w))
        .zip(scratch.q.par_chunks_mut(BLOCK * w))
        .enumerate()
        .for_each(|(br, ((ys, is), qs))| {
            let y0 = br * BLOCK;
            let rows = ys.len() / w;
            for bx in (0..w).step_by(BLOCK) {
                let cols = BLOCK.min(w - bx);
                // Motion search: candidates in row-major order from (0, 0).
                let sad = |dx: isize, dy: isize| -> f32 {
                    let mut acc = 0.0;
                    for ry in (0..rows).step_by(2) {
                        let py = (y0 + ry) as isize;
                        for rx in (0..cols).step_by(2) {
                            let px = (bx + rx) as isize;
                            let c = cur.y[py as usize * w + px as usize];
                            acc += (c - prev.input.y[idx(px + dx, py + dy)]).abs();
                        }
                    }
                    acc
                };
                let (mut best, mut best_sad) = ((0isize, 0isize), sad(0, 0));
                let span = search as isize;
                for dy in (-span..=span).step_by(st) {
                    for dx in (-span..=span).step_by(st) {
                        if (dx, dy) == (0, 0) {
                            continue;
                        }
                        let v = sad(dx, dy);
                        if v < best_sad {
                            best_sad = v;
                            best = (dx, dy);
                        }
                    }
                }
                let (dx, dy) = best;
                let vx = (gain * dx as f32).round() as isize;
                let vy = (gain * dy as f32).round() as isize;
                for ry in 0..rows {
                    let py = (y0 + ry) as isize;
                    for rx in 0..cols {
                        let px = (bx + rx) as isize;
                        let p = py as usize * w + px as usize;
                        let m = idx(px + dx, py + dy);
                        let o = idx(px + vx, py + vy);
                        let k = ry * w + bx + rx;
                        ys[k] = prev.output.y[o] + residual * (cur.y[p] - prev.input.y[m]);
                        is[k] = prev.output.i[o] + residual * (cur.i[p] - prev.input.i[m]);
                        qs[k] = prev.output.q[o] + residual * (cur.q[p] - prev.input.q[m]);
                    }
                }
            }
        });

    frames.input.copy_from(planes);
    std::mem::swap(planes, scratch);
    frames.output.copy_from(planes);
}
