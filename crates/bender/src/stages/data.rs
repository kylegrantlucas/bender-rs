//! Bridged ADC/RAM data bus: 8-bit sample codes get bits bridged, stuck or
//! latched late.
//!
//! A solder bridge between two data lines makes the bits trade places, one
//! drive the other, or read as a wired AND/OR. A slow latch on the high bits
//! captures them from an earlier sample than the low bits, so edges smear in
//! steps of the most significant bits.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{DataBend, DataTarget, PairMode, StageId, StuckMode};
use crate::stages::stage_rng;

const BAND: usize = 8;

/// Corrupts quantised sample codes in place, per 8-row band of contact.
///
/// Each row is processed right to left: the high bits (at or above
/// `lag_bit`) come from the original code `lag` pixels to the left, then the
/// bridged pair is applied, then the stuck bit.
pub(crate) fn apply(s: &DataBend, seed: i32, n: u64, planes: &mut YiqPlanes) {
    let w = planes.width;
    if w == 0 || planes.height == 0 {
        return;
    }
    let (sa, sb) = (s.swap_a.clamp(0, 7) as u32, s.swap_b.clamp(0, 7) as u32);
    let pair_mode = s.pair_mode;
    let stuck = 1u32 << s.stuck_bit.clamp(0, 7);
    let mode = s.stuck_mode;
    let hi_mask = (0xFFu32 << s.lag_bit.clamp(0, 8)) & 0xFF;
    let lag = s.lag.max(1) as usize;
    let bend = move |code: u32| -> u32 {
        let mut c = code;
        if sa != sb {
            let (ba, bb) = ((c >> sa) & 1, (c >> sb) & 1);
            let (na, nb) = match pair_mode {
                PairMode::Swap => (bb, ba),
                PairMode::Copy => (ba, ba),
                PairMode::And => (ba & bb, ba & bb),
                PairMode::Or => (ba | bb, ba | bb),
            };
            c = (c & !((1 << sa) | (1 << sb))) | (na << sa) | (nb << sb);
        }
        match mode {
            StuckMode::Off => c,
            StuckMode::Low => c & !stuck,
            StuckMode::High => c | stuck,
            StuckMode::Invert => c ^ stuck,
        }
    };
    let luma = Codec {
        encode: |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u32,
        decode: |c: u32| c as f32 / 255.0,
    };
    let chroma = Codec {
        encode: |v: f32| ((v + 0.6) / 1.2 * 255.0).round().clamp(0.0, 255.0) as u32,
        decode: |c: u32| c as f32 / 255.0 * 1.2 - 0.6,
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
            if !stage_rng(seed, StageId::Data, n)
                .mix(band as u64)
                .chance(s.coverage)
            {
                return;
            }
            for ((yr, ir), qr) in ys.chunks_mut(w).zip(is.chunks_mut(w)).zip(qs.chunks_mut(w)) {
                if do_luma {
                    luma.bend_row(yr, hi_mask, lag, bend);
                }
                if do_chroma {
                    chroma.bend_row(ir, hi_mask, lag, bend);
                    chroma.bend_row(qr, hi_mask, lag, bend);
                }
            }
        });
}

/// Mapping between a plane's float values and 8-bit sample codes.
struct Codec<E, D> {
    encode: E,
    decode: D,
}

impl<E: Fn(f32) -> u32, D: Fn(u32) -> f32> Codec<E, D> {
    /// Bends one row right to left, so samples to the left of `x` still hold
    /// their original codes when the late-latched high bits are read.
    fn bend_row(&self, row: &mut [f32], hi_mask: u32, lag: usize, bend: impl Fn(u32) -> u32) {
        for x in (0..row.len()).rev() {
            let own = (self.encode)(row[x]);
            let code = if hi_mask == 0 {
                own
            } else {
                let early = (self.encode)(row[x.saturating_sub(lag)]);
                (early & hi_mask) | (own & !hi_mask)
            };
            row[x] = (self.decode)(bend(code));
        }
    }
}
