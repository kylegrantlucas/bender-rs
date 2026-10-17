//! Small media helpers: element creation, still detection, sizing, frame numbers.

use std::path::Path;

use gstreamer as gst;

use crate::GstError;

/// Creates an element by factory name, reporting which plugin package is missing.
pub fn make(name: &str) -> Result<gst::Element, GstError> {
    if gst::ElementFactory::find(name).is_none() {
        let package = match name {
            "x264enc" => "gst-plugins-ugly",
            "avenc_aac" | "avenc_ffv1" => "gst-libav",
            "mp4mux" | "matroskamux" | "pngenc" | "flacenc" | "imagefreeze" => "gst-plugins-good",
            _ => "gst-plugins-base",
        };
        return Err(GstError::MissingElement {
            name: name.to_owned(),
            package,
        });
    }
    Ok(gst::ElementFactory::make(name).build()?)
}

/// Whether `path` has a still-image extension.
pub fn is_still_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| {
            matches!(
                e.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff"
            )
        })
}

/// Processing size for a source of `src_w`x`src_h` scaled to `scanlines` rows
/// (`0` keeps the native size); both dimensions are rounded to even, at least 2.
pub fn scaled_size(src_w: i32, src_h: i32, scanlines: u32) -> (i32, i32) {
    let (w, h) = if scanlines == 0 || src_h <= 0 {
        (f64::from(src_w), f64::from(src_h))
    } else {
        let s = f64::from(scanlines);
        (f64::from(src_w) * s / f64::from(src_h), s)
    };
    let even = |v: f64| (2.0 * (v / 2.0).round()).max(2.0) as i32;
    (even(w), even(h))
}

/// Frame index for a buffer timestamp at `fps`; `None` without a pts or framerate.
pub fn frame_number(pts: Option<gst::ClockTime>, fps: gst::Fraction) -> Option<u64> {
    let pts = pts?;
    if fps.numer() <= 0 || fps.denom() <= 0 {
        return None;
    }
    let n = fps.numer() as u128 * (u128::from(pts.nseconds()) + 100) / fps.denom() as u128;
    Some((n / 1_000_000_000) as u64)
}
