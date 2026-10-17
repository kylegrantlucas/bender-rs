//! Error type shared by the GStreamer integration.

use gst::glib;
use gstreamer as gst;

/// Errors raised while building or running pipelines.
#[derive(Debug, thiserror::Error)]
pub enum GstError {
    /// A GLib error (e.g. GStreamer initialisation).
    #[error(transparent)]
    Glib(#[from] glib::Error),
    /// A failed GLib/GStreamer call.
    #[error(transparent)]
    Bool(#[from] glib::BoolError),
    /// A pipeline state change failed.
    #[error(transparent)]
    StateChange(#[from] gst::StateChangeError),
    /// A required element factory is not installed.
    #[error("missing GStreamer element '{name}' (install {package})")]
    MissingElement {
        /// Element factory name.
        name: String,
        /// Plugin package that provides it.
        package: &'static str,
    },
    /// The requested input/output combination is not supported.
    #[error("{0}")]
    Unsupported(String),
    /// An error reported by a running pipeline.
    #[error("{0}")]
    Pipeline(String),
}
