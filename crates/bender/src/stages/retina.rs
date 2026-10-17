//! Game Boy Camera retina (Mitsubishi M64282FP + MAC-GBD): the picture is
//! reduced to a tiny monochrome sensor grid, edge-enhanced by the sensor's
//! analogue filter, exposed by a slow read clock, quantized through a 4×4
//! ordered dither to a few grey levels and shown on a DMG palette.

use std::sync::LazyLock;

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{RetinaCamera, RetinaEdge, RetinaPalette};

/// Edge ratio register values.
const ALPHA: [f32; 8] = [0.5, 0.75, 1.0, 1.25, 2.0, 3.0, 4.0, 5.0];

/// 4×4 Bayer ordered-dither matrix.
const BAYER: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

/// DMG greens #0f380f, #306230, #8bac0f, #9bbc0f as YIQ.
static DMG: LazyLock<[[f32; 3]; 4]> = LazyLock::new(|| {
    const RGB: [[u8; 3]; 4] = [
        [0x0f, 0x38, 0x0f],
        [0x30, 0x62, 0x30],
        [0x8b, 0xac, 0x0f],
        [0x9b, 0xbc, 0x0f],
    ];
    RGB.map(|[r, g, b]| {
        let (r, g, b) = (
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        );
        [
            0.299 * r + 0.587 * g + 0.114 * b,
            0.5959 * r - 0.2746 * g - 0.3213 * b,
            0.2115 * r - 0.5227 * g + 0.3112 * b,
        ]
    })
});

/// Renders the picture through the retina camera in place.
pub(crate) fn apply(s: &RetinaCamera, planes: &mut YiqPlanes, buf: &mut Vec<f32>) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    let res = s.resolution.max(1) as f32;
    let b = ((h as f32 / res).round() as usize).max(1);
    let (gw, gh) = (w.div_ceil(b), h.div_ceil(b));
    let n = gw * gh;
    buf.clear();
    buf.resize(2 * n, 0.0);
    let (grid, tmp) = buf.split_at_mut(n);

    // 1–2. Block-average luma into the sensor grid, optionally inverted.
    let src = &planes.y;
    let invert = s.invert;
    grid.par_chunks_mut(gw).enumerate().for_each(|(gy, row)| {
        let y0 = gy * b;
        let y1 = (y0 + b).min(h);
        for (gx, cell) in row.iter_mut().enumerate() {
            let x0 = gx * b;
            let x1 = (x0 + b).min(w);
            let mut sum = 0.0;
            for yy in y0..y1 {
                sum += src[yy * w + x0..yy * w + x1].iter().sum::<f32>();
            }
            let v = sum / ((y1 - y0) * (x1 - x0)) as f32;
            *cell = if invert { 1.0 - v } else { v };
        }
    });

    // 3–5. Edge filter, slow read and ordered-dither quantization.
    let alpha = ALPHA[s.edge.clamp(0, 7) as usize];
    let mode = s.edge_mode;
    let levels = s.levels.max(2) as f32;
    let (dither, slow) = (s.dither, s.read_slow);
    let grid_ro: &[f32] = grid;
    tmp.par_chunks_mut(gw).enumerate().for_each(|(gy, row)| {
        let at = |x: usize, y: usize| grid_ro[y * gw + x];
        let up = gy.saturating_sub(1);
        let down = (gy + 1).min(gh - 1);
        for (gx, out) in row.iter_mut().enumerate() {
            let p = at(gx, gy);
            let west = at(gx.saturating_sub(1), gy);
            let east = at((gx + 1).min(gw - 1), gy);
            let mut g = match mode {
                RetinaEdge::TwoD => p + alpha * (4.0 * p - at(gx, up) - at(gx, down) - east - west),
                RetinaEdge::Horizontal => p + alpha * (2.0 * p - west - east),
                RetinaEdge::Off => p,
            };
            g *= 1.0 + slow * 2.0 * (gy as f32 / gh as f32);
            let t =
                g * (levels - 1.0) + dither * (BAYER[gy & 3][gx & 3] + 0.5) / 16.0 - dither * 0.5;
            let lvl = (t + 0.5).floor().clamp(0.0, levels - 1.0);
            *out = lvl / (levels - 1.0);
        }
    });

    // 6. Palette, written over every pixel of each block.
    let tmp_ro: &[f32] = tmp;
    let palette = s.palette;
    let dmg = &*DMG;
    planes
        .y
        .par_chunks_mut(w)
        .zip(planes.i.par_chunks_mut(w))
        .zip(planes.q.par_chunks_mut(w))
        .enumerate()
        .for_each(|(r, ((yr, ir), qr))| {
            let cells = &tmp_ro[(r / b) * gw..(r / b + 1) * gw];
            for x in 0..w {
                let v = cells[x / b];
                let (cy, ci, cq) = match palette {
                    RetinaPalette::Grey => (v, 0.0, 0.0),
                    RetinaPalette::Dmg => {
                        let t = v * 3.0;
                        let k = (t.floor() as usize).min(2);
                        let f = t - k as f32;
                        let (a, c) = (dmg[k], dmg[k + 1]);
                        (
                            a[0] + (c[0] - a[0]) * f,
                            a[1] + (c[1] - a[1]) * f,
                            a[2] + (c[2] - a[2]) * f,
                        )
                    }
                };
                yr[x] = cy;
                ir[x] = ci;
                qr[x] = cq;
            }
        });
}
