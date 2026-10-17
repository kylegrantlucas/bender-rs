//! Rewired processing amplifier: inverted luma/chroma, swapped I/Q, or
//! solarised highlights, blended with the clean signal.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{Inversion, InvertMode};

/// Applies the inversion in place.
pub(crate) fn apply(s: &Inversion, planes: &mut YiqPlanes) {
    let (mode, t, mix) = (s.mode, s.threshold, s.mix);
    planes
        .y
        .par_iter_mut()
        .zip(planes.i.par_iter_mut())
        .zip(planes.q.par_iter_mut())
        .for_each(|((y, i), q)| {
            let (oy, oi, oq) = (*y, *i, *q);
            let (ny, ni, nq) = match mode {
                InvertMode::InvertLuma => (1.0 - oy, oi, oq),
                InvertMode::InvertChroma => (oy, -oi, -oq),
                InvertMode::SwapIq => (oy, oq, oi),
                InvertMode::Solarize if oy > t => (1.0 - oy, -oi, -oq),
                InvertMode::Solarize => (oy, oi, oq),
            };
            *y = oy + (ny - oy) * mix;
            *i = oi + (ni - oi) * mix;
            *q = oq + (nq - oq) * mix;
        });
}
