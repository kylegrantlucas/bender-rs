//! One-time GStreamer initialisation and element registration.

use std::sync::OnceLock;

use gst::glib;
use gst::prelude::*;
use gstreamer as gst;

use crate::{BenderFilterElement, GstError};

/// Initialises GStreamer and registers `benderfilter`; safe to call repeatedly.
pub fn init() -> Result<(), GstError> {
    static RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    RESULT
        .get_or_init(|| {
            gst::init().map_err(|e| e.to_string())?;
            gst::Element::register(
                None,
                "benderfilter",
                gst::Rank::NONE,
                BenderFilterElement::static_type(),
            )
            .map_err(|e: glib::BoolError| e.to_string())
        })
        .clone()
        .map_err(GstError::Pipeline)
}
