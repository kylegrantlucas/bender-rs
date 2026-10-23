//! Circuit-bent video hardware emulation on YIQ float planes.
//!
//! [`BendEffect`] holds every setting and *is* the effect: call
//! [`BendEffect::apply_rgb8`], [`BendEffect::apply_rgba8`] or
//! [`BendEffect::apply_yiq`] with a per-stream [`BendState`].

pub mod effect;
pub mod planes;
pub mod random;
pub mod settings;
mod stages;

pub use effect::BendState;
pub use planes::{PixelOrder, YiqPlanes};
pub use settings::*;
