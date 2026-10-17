//! An 8×8 DCT block codec (JPEG/MPEG intra style) with bent hardware: the
//! quantizer ROM address is offset, coefficients are read back in a rotated
//! zig-zag order, the DC predictor slips for the rest of a block row, and
//! the macroblock address generator sometimes fetches the wrong macroblock.

use std::sync::LazyLock;

use rayon::prelude::*;

use crate::planes::YiqPlanes;
use crate::settings::{DctCodec, StageId};
use crate::stages::stage_rng;

/// IJG standard luminance quantization table, natural order.
const LUMA_TABLE: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// IJG standard chrominance quantization table, natural order.
const CHROMA_TABLE: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

/// Zig-zag scan: zig-zag index → natural (row-major) index.
const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Orthonormal DCT-II basis: `COS[k][x] = c(k)·cos((2x+1)kπ/16)`.
static COS: LazyLock<[[f32; 8]; 8]> = LazyLock::new(|| {
    let mut t = [[0.0; 8]; 8];
    for (k, row) in t.iter_mut().enumerate() {
        let c = if k == 0 { (1.0f32 / 8.0).sqrt() } else { 0.5 };
        for (x, v) in row.iter_mut().enumerate() {
            *v = c * ((2 * x + 1) as f32 * k as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }
    t
});

/// Scales an IJG table to a quality setting.
fn scaled_table(table: &[u8; 64], quality: i32) -> [f32; 64] {
    let q = quality.clamp(1, 100);
    let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
    let mut out = [0.0; 64];
    for (o, &t) in out.iter_mut().zip(table) {
        *o = ((t as i32 * scale + 50) / 100).clamp(1, 255) as f32;
    }
    out
}

/// Forward (`inverse = false`) or inverse separable 8×8 DCT, in place.
fn dct8x8(b: &mut [f32; 64], inverse: bool) {
    let c = &*COS;
    let mut t = [0.0f32; 64];
    // Rows.
    for r in 0..8 {
        for k in 0..8 {
            let mut acc = 0.0;
            for x in 0..8 {
                acc += if inverse {
                    c[x][k] * b[r * 8 + x]
                } else {
                    c[k][x] * b[r * 8 + x]
                };
            }
            t[r * 8 + k] = acc;
        }
    }
    // Columns.
    for col in 0..8 {
        for k in 0..8 {
            let mut acc = 0.0;
            for y in 0..8 {
                acc += if inverse {
                    c[y][k] * t[y * 8 + col]
                } else {
                    c[k][y] * t[y * 8 + col]
                };
            }
            b[k * 8 + col] = acc;
        }
    }
}

/// Encodes and decodes one plane of codes in place, parallel over block rows.
#[allow(clippy::too_many_arguments)]
fn codec_plane(
    data: &mut [f32],
    pw: usize,
    ph: usize,
    plane: u64,
    table: &[f32; 64],
    s: &DctCodec,
    seed: i32,
    n: u64,
) {
    if pw == 0 || ph == 0 {
        return;
    }
    let bw = pw.div_ceil(8);
    let shift = s.table_shift.rem_euclid(64) as usize;
    let steps: [f32; 64] = std::array::from_fn(|z| table[ZIGZAG[(z + shift) % 64] as usize]);
    data.par_chunks_mut(8 * pw)
        .enumerate()
        .for_each(|(br, band)| {
            let rows = band.len() / pw;
            let mut rng = stage_rng(seed, StageId::Dct, n).mix(plane).mix(br as u64);
            let (b0, delta) = if rng.chance(s.dc_drift) {
                let b0 = (rng.next_u64() % bw as u64) as usize;
                (b0, rng.range(-48.0, 48.0))
            } else {
                (usize::MAX, 0.0)
            };
            for bx in 0..bw {
                let x0 = bx * 8;
                let mut blk = [0.0f32; 64];
                for (yy, brow) in blk.as_chunks_mut::<8>().0.iter_mut().enumerate() {
                    let row = &band[yy.min(rows - 1) * pw..][..pw];
                    for (xx, v) in brow.iter_mut().enumerate() {
                        *v = row[(x0 + xx).min(pw - 1)];
                    }
                }
                dct8x8(&mut blk, false);
                let mut qz = [0.0f32; 64];
                for (z, q) in qz.iter_mut().enumerate() {
                    *q = (blk[ZIGZAG[z] as usize] / steps[z]).round();
                }
                if rng.chance(s.scramble) {
                    let r = 1 + (rng.next_u64() % 62) as usize;
                    qz[1..].rotate_right(r);
                }
                for (z, &q) in qz.iter().enumerate() {
                    blk[ZIGZAG[z] as usize] = q * steps[z];
                }
                if bx >= b0 {
                    blk[0] += 8.0 * delta;
                }
                dct8x8(&mut blk, true);
                for (yy, brow) in blk.as_chunks::<8>().0.iter().enumerate().take(rows) {
                    let row = &mut band[yy * pw..][..pw];
                    let len = 8.min(pw - x0);
                    row[x0..x0 + len].copy_from_slice(&brow[..len]);
                }
            }
        });
}

/// Runs the picture through the bent DCT codec.
pub(crate) fn apply(
    s: &DctCodec,
    seed: i32,
    n: u64,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
    buf: &mut Vec<f32>,
) {
    let (w, h) = (planes.width, planes.height);
    if w == 0 || h == 0 {
        return;
    }
    let luma = scaled_table(&LUMA_TABLE, s.quality);
    let chroma = scaled_table(&CHROMA_TABLE, s.quality);

    // Luma.
    planes
        .y
        .par_iter_mut()
        .for_each(|v| *v = *v * 255.0 - 128.0);
    codec_plane(&mut planes.y, w, h, 0, &luma, s, seed, n);
    planes
        .y
        .par_iter_mut()
        .for_each(|v| *v = (*v + 128.0) / 255.0);

    // Chroma at 4:2:0.
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let len = cw * ch;
    buf.clear();
    buf.resize(2 * len, 0.0);
    let (bi, bq) = buf.split_at_mut(len);
    let to_code = 255.0 / 1.2;
    for (sub, src) in [(&mut *bi, &planes.i), (&mut *bq, &planes.q)] {
        sub.par_chunks_mut(cw).enumerate().for_each(|(cy, row)| {
            let (y0, y1) = (2 * cy, (2 * cy + 1).min(h - 1));
            for (cx, v) in row.iter_mut().enumerate() {
                let (x0, x1) = (2 * cx, (2 * cx + 1).min(w - 1));
                let sum = src[y0 * w + x0] + src[y0 * w + x1] + src[y1 * w + x0] + src[y1 * w + x1];
                *v = sum * 0.25 * to_code;
            }
        });
    }
    codec_plane(bi, cw, ch, 1, &chroma, s, seed, n);
    codec_plane(bq, cw, ch, 2, &chroma, s, seed, n);
    for (dst, sub) in [(&mut planes.i, &*bi), (&mut planes.q, &*bq)] {
        dst.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let srow = &sub[(y / 2) * cw..][..cw];
            for (x, v) in row.iter_mut().enumerate() {
                *v = srow[x / 2] / to_code;
            }
        });
    }

    // Macroblock address errors.
    if s.block_shuffle <= 0.0 {
        return;
    }
    scratch.width = w;
    scratch.height = h;
    scratch.y.clone_from(&planes.y);
    scratch.i.clone_from(&planes.i);
    scratch.q.clone_from(&planes.q);
    let mbw = w.div_ceil(16);
    let src = &*scratch;
    planes
        .y
        .par_chunks_mut(16 * w)
        .zip(planes.i.par_chunks_mut(16 * w))
        .zip(planes.q.par_chunks_mut(16 * w))
        .enumerate()
        .for_each(|(my, ((y, i), q))| {
            let rows = y.len() / w;
            let base = my * 16 * w;
            for mx in 0..mbw {
                let mut rng = stage_rng(seed, StageId::Dct, n)
                    .mix(99)
                    .mix((my * mbw + mx) as u64);
                if !rng.chance(s.block_shuffle) {
                    continue;
                }
                let sx = (rng.next_u64() % mbw as u64) as usize * 16;
                let dx = mx * 16;
                let cols = 16.min(w - dx).min(w - sx);
                for r in 0..rows {
                    let (d, o) = (r * w + dx, base + r * w + sx);
                    y[d..d + cols].copy_from_slice(&src.y[o..o + cols]);
                    i[d..d + cols].copy_from_slice(&src.i[o..o + cols]);
                    q[d..d + cols].copy_from_slice(&src.q[o..o + cols]);
                }
            }
        });
}
