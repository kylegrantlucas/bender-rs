//! Bridged ADC/RAM data bus: 8-bit sample codes get bits swapped or stuck.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{DataBend, DataTarget, StuckMode};
use crate::stages::{DATA, stage_rng};

const BAND: usize = 8;

/// Corrupts quantised sample codes in place, per 8-row band of contact.
pub(crate) fn apply(s: &DataBend, seed: i32, n: u64, planes: &mut YiqPlanes) {
    let w = planes.width;
    if w == 0 || planes.height == 0 {
        return;
    }
    let (sa, sb) = (s.swap_a.clamp(0, 7) as u32, s.swap_b.clamp(0, 7) as u32);
    let stuck = 1u32 << s.stuck_bit.clamp(0, 7);
    let mode = s.stuck_mode;
    let bend = move |code: u32| -> u32 {
        let mut c = code;
        if sa != sb {
            let (ba, bb) = ((c >> sa) & 1, (c >> sb) & 1);
            c = (c & !((1 << sa) | (1 << sb))) | (ba << sb) | (bb << sa);
        }
        match mode {
            StuckMode::Off => c,
            StuckMode::Low => c & !stuck,
            StuckMode::High => c | stuck,
            StuckMode::Invert => c ^ stuck,
        }
    };
    let luma = |v: &mut f32| {
        let code = (*v * 255.0).round().clamp(0.0, 255.0) as u32;
        *v = bend(code) as f32 / 255.0;
    };
    let chroma = |v: &mut f32| {
        let code = ((*v + 0.6) / 1.2 * 255.0).round().clamp(0.0, 255.0) as u32;
        *v = bend(code) as f32 / 255.0 * 1.2 - 0.6;
    };
    let (do_luma, do_chroma) = match s.target {
        DataTarget::Luma => (true, false),
        DataTarget::Chroma => (false, true),
        DataTarget::Both => (true, true),
    };

    planes
        .y
        .par_chunks_mut(w * BAND)
        .zip(planes.i.par_chunks_mut(w * BAND))
        .zip(planes.q.par_chunks_mut(w * BAND))
        .enumerate()
        .for_each(|(band, ((ys, is), qs))| {
            if !stage_rng(seed, DATA, n).mix(band as u64).chance(s.coverage) {
                return;
            }
            if do_luma {
                ys.iter_mut().for_each(luma);
            }
            if do_chroma {
                is.iter_mut().for_each(chroma);
                qs.iter_mut().for_each(chroma);
            }
        });
}
