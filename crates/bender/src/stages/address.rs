//! Bridged frame-store read address lines: one address bit is forced or
//! combined with a partner bit, shuffling which pixel is read back.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{AddressBridge, Axis, BridgeMode};
use crate::stages::stage_rng;

const BAND: usize = 8;

/// Reads each output pixel from a bit-altered address, gathering into `scratch`.
pub(crate) fn apply(
    s: &AddressBridge,
    seed: i32,
    stage_id: u64,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);
    let bit = s.bit.clamp(0, 10) as u32;
    let pbit = s.partner_bit.clamp(0, 10) as u32;
    let (axis, paxis, mode) = (s.axis, s.partner_axis, s.mode);
    let alter = move |x: usize, y: usize| -> (usize, usize) {
        let pick = |a: Axis| if a == Axis::Column { x } else { y };
        let c = pick(axis);
        let p = (pick(paxis) >> pbit) & 1;
        let b = (c >> bit) & 1;
        let nb = match mode {
            BridgeMode::StuckLow => 0,
            BridgeMode::StuckHigh => 1,
            BridgeMode::Xor => b ^ p,
            BridgeMode::And => b & p,
            BridgeMode::Or => b | p,
        };
        let c2 = (c & !(1 << bit)) | (nb << bit);
        match axis {
            Axis::Column => (c2 % w, y),
            Axis::Row => (x, c2 % h),
        }
    };

    let src = &*planes;
    scratch
        .y
        .par_chunks_mut(w)
        .zip(scratch.i.par_chunks_mut(w))
        .zip(scratch.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yo, io), qo))| {
            let active = stage_rng(seed, stage_id, n)
                .mix((r / BAND) as u64)
                .chance(s.coverage);
            for x in 0..w {
                let (sx, sy) = if active { alter(x, r) } else { (x, r) };
                let k = sy * w + sx;
                yo[x] = src.y[k];
                io[x] = src.i[k];
                qo[x] = src.q[k];
            }
        });
    std::mem::swap(planes, scratch);
}
