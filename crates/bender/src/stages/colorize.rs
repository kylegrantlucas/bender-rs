//! Video-synth colourizer and luma keyer: luma is folded by frequency
//! doublers, sliced into bands (or swept) and painted with hues.

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{ColorizeMode, Colorizer};

/// Colourizes the planes in place.
pub(crate) fn apply(s: &Colorizer, planes: &mut YiqPlanes) {
    if planes.width == 0 || planes.height == 0 {
        return;
    }
    let mode = s.mode;
    let bands = s.bands.max(1) as f32;
    let folds = s.fold.max(0);
    let (hue, spread, sat, key, mix) = (s.hue, s.spread, s.saturation, s.key, s.mix);
    planes
        .y
        .par_iter_mut()
        .zip(planes.i.par_iter_mut())
        .zip(planes.q.par_iter_mut())
        .for_each(|((y, i), q)| {
            let (oy, oi, oq) = (*y, *i, *q);
            if oy < key {
                return;
            }
            let mut v = oy;
            for _ in 0..folds {
                v = if v < 0.5 { 2.0 * v } else { 2.0 * (1.0 - v) };
            }
            let (yc, theta) = match mode {
                ColorizeMode::Bands => {
                    let k = (v * bands).floor().clamp(0.0, bands - 1.0);
                    ((k + 0.5) / bands, hue + k * spread * 360.0 / bands)
                }
                ColorizeMode::Gradient => (v, hue + v * spread * 360.0),
            };
            let (sin, cos) = theta.to_radians().sin_cos();
            let (ic, qc) = (0.5 * sat * cos, 0.5 * sat * sin);
            *y = oy + (yc - oy) * mix;
            *i = oi + (ic - oi) * mix;
            *q = oq + (qc - oq) * mix;
        });
}
