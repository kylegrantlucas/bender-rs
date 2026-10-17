//! Starved supply rail: bright rows sag in gain and width, highlights smear
//! to the right, and mains hum rolls a dark bar through the picture.

use std::f32::consts::TAU;

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::RailSag;

/// Applies width breathing (gathering into `scratch`), gain sag, streak and hum.
pub(crate) fn apply(s: &RailSag, n: u64, planes: &mut YiqPlanes, scratch: &mut YiqPlanes) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    scratch.resize(w, h);
    let half = w as f32 / 2.0;
    let k = (s.streak > 0.0).then(|| (-1.0f32 / (s.streak * w as f32 * 0.25)).exp());
    let frame_phase = (n as f64 * 0.01).rem_euclid(1.0) as f32;

    let src = &*planes;
    scratch
        .y
        .par_chunks_mut(w)
        .zip(scratch.i.par_chunks_mut(w))
        .zip(scratch.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yo, io), qo))| {
            let range = r * w..(r + 1) * w;
            let (ys, is, qs) = (&src.y[range.clone()], &src.i[range.clone()], &src.q[range]);
            let m = ys.iter().sum::<f32>() / w as f32;
            let gain = 1.0 - 0.5 * s.sag * m;
            let scale = 1.0 - 0.15 * s.sag * m;
            for x in 0..w {
                let sx = (x as f32 - half) / scale + half;
                if sx < 0.0 || sx > (w - 1) as f32 {
                    yo[x] = 0.0;
                    io[x] = 0.0;
                    qo[x] = 0.0;
                    continue;
                }
                let x0 = sx.floor() as usize;
                let x1 = (x0 + 1).min(w - 1);
                let f = sx - x0 as f32;
                let lerp = |row: &[f32]| row[x0] + (row[x1] - row[x0]) * f;
                yo[x] = lerp(ys) * gain;
                io[x] = lerp(is);
                qo[x] = lerp(qs);
            }
            if let Some(k) = k {
                let mut acc = 0.0f32;
                for y in yo.iter_mut() {
                    acc = (*y - 0.7).max(0.0).max(acc * k);
                    *y += 0.6 * s.streak * acc;
                }
            }
            if s.hum > 0.0 {
                let wave = 0.5 + 0.5 * (TAU * (r as f32 / h as f32 + frame_phase)).sin();
                let hum = 1.0 - 0.3 * s.hum * wave;
                yo.iter_mut().for_each(|y| *y *= hum);
            }
        });
    std::mem::swap(planes, scratch);
}
