//! GStreamer integration for bender-rs: the `benderfilter` element plus
//! preview and render pipelines.

pub use gstreamer as gst;

pub mod decode;
pub mod error;
mod filter;
mod init;
pub mod media;
pub mod preview;
pub mod render;

pub use error::GstError;
pub use filter::BendSettingsBoxed;
pub use init::init;
pub use preview::{PreviewFrame, PreviewPipeline};
pub use render::{OutputCodec, RenderJob, RenderSettings, RenderStatus};

use gst::glib;

glib::wrapper! {
    /// The `benderfilter` element: applies a [`bender::BendEffect`] to raw RGBA/BGRA video.
    pub struct BenderFilterElement(ObjectSubclass<filter::BenderFilter>)
        @extends gstreamer_video::VideoFilter, gstreamer_base::BaseTransform, gst::Element, gst::Object;
}
