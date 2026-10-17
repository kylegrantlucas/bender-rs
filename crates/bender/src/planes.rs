//! Planar YIQ frame buffers and RGBA8 conversion.
//!
//! Composite video carries luma (Y) plus two chroma components (I, Q); bending
//! happens in this domain, as it would inside the real hardware.

use rayon::prelude::*;

/// Byte order of 4-byte packed pixels. The alpha/padding byte is index 3 in both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelOrder {
    /// R, G, B, A.
    Rgba,
    /// B, G, R, A.
    Bgra,
}

impl PixelOrder {
    fn rb(self) -> (usize, usize) {
        match self {
            PixelOrder::Rgba => (0, 2),
            PixelOrder::Bgra => (2, 0),
        }
    }
}

/// Row-major Y, I and Q float planes, each `width * height` long.
#[derive(Clone, Debug, Default)]
pub struct YiqPlanes {
    pub width: usize,
    pub height: usize,
    pub y: Vec<f32>,
    pub i: Vec<f32>,
    pub q: Vec<f32>,
}

impl YiqPlanes {
    /// Allocates zeroed planes.
    pub fn new(width: usize, height: usize) -> Self {
        let mut p = Self::default();
        p.resize(width, height);
        p
    }

    /// Resizes the planes, reusing their allocations. Contents are unspecified afterwards.
    pub fn resize(&mut self, width: usize, height: usize) {
        let len = width * height;
        self.width = width;
        self.height = height;
        self.y.resize(len, 0.0);
        self.i.resize(len, 0.0);
        self.q.resize(len, 0.0);
    }

    /// Copies dimensions and contents from `other`.
    pub fn copy_from(&mut self, other: &YiqPlanes) {
        self.width = other.width;
        self.height = other.height;
        self.y.clone_from(&other.y);
        self.i.clone_from(&other.i);
        self.q.clone_from(&other.q);
    }

    /// Loads gamma-encoded 8-bit pixels and converts them to FCC YIQ.
    ///
    /// # Panics
    /// If `src` is too short for `height` rows of `stride` bytes.
    pub fn load_rgba8(
        &mut self,
        width: usize,
        height: usize,
        src: &[u8],
        stride: usize,
        order: PixelOrder,
    ) {
        self.resize(width, height);
        if width == 0 || height == 0 {
            return;
        }
        let (ri, bi) = order.rb();
        self.y
            .par_chunks_mut(width)
            .zip(self.i.par_chunks_mut(width))
            .zip(self.q.par_chunks_mut(width))
            .enumerate()
            .for_each(|(r, ((yr, ir), qr))| {
                let row = &src[r * stride..r * stride + width * 4];
                for (x, px) in row.as_chunks::<4>().0.iter().enumerate() {
                    let red = f32::from(px[ri]) / 255.0;
                    let g = f32::from(px[1]) / 255.0;
                    let b = f32::from(px[bi]) / 255.0;
                    yr[x] = 0.299 * red + 0.587 * g + 0.114 * b;
                    ir[x] = 0.5959 * red - 0.2746 * g - 0.3213 * b;
                    qr[x] = 0.2115 * red - 0.5227 * g + 0.3112 * b;
                }
            });
    }

    /// Converts back to 8-bit pixels, clamping RGB to `[0, 1]` and writing alpha 255.
    ///
    /// # Panics
    /// If `dst` is too short for `height` rows of `stride` bytes.
    pub fn store_rgba8(&self, dst: &mut [u8], stride: usize, order: PixelOrder) {
        let width = self.width;
        if width == 0 || self.height == 0 {
            return;
        }
        let (ri, bi) = order.rb();
        let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        dst.par_chunks_mut(stride)
            .take(self.height)
            .enumerate()
            .for_each(|(r, row)| {
                let base = r * width;
                let yr = &self.y[base..base + width];
                let ir = &self.i[base..base + width];
                let qr = &self.q[base..base + width];
                for (x, px) in row[..width * 4]
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .enumerate()
                {
                    let (y, i, q) = (yr[x], ir[x], qr[x]);
                    px[ri] = to_u8(y + 0.956 * i + 0.619 * q);
                    px[1] = to_u8(y - 0.272 * i - 0.647 * q);
                    px[bi] = to_u8(y - 1.106 * i + 1.703 * q);
                    px[3] = 255;
                }
            });
    }
}
