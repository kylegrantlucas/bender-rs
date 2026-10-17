//! `benderfilter` VideoFilter implementation.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use bender::{BendEffect, BendState, PixelOrder};
use gst::glib;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_base::subclass::BaseTransformMode;
use gstreamer_base::subclass::prelude::*;
use gstreamer_video::subclass::prelude::*;
use gstreamer_video::{VideoFormat, VideoFrameExt, VideoFrameRef, VideoInfo};

use crate::media::frame_number;

/// [`BendEffect`] wrapped as a GLib boxed type for the `settings` property.
#[derive(Clone, Debug, Default, glib::Boxed)]
#[boxed_type(name = "BendSettingsBoxed")]
pub struct BendSettingsBoxed(pub BendEffect);

#[derive(Default)]
pub struct BenderFilter {
    settings: Mutex<BendEffect>,
    state: Mutex<BendState>,
    info: Mutex<Option<VideoInfo>>,
    counter: AtomicU64,
}

#[glib::object_subclass]
impl ObjectSubclass for BenderFilter {
    const NAME: &'static str = "BenderFilter";
    type Type = crate::BenderFilterElement;
    type ParentType = gstreamer_video::VideoFilter;
}

impl ObjectImpl for BenderFilter {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPS: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
        PROPS.get_or_init(|| {
            vec![
                glib::ParamSpecBoxed::builder::<BendSettingsBoxed>("settings")
                    .nick("Settings")
                    .blurb("Circuit-bend effect settings")
                    .mutable_playing()
                    .build(),
            ]
        })
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        if pspec.name() == "settings" {
            let boxed = value
                .get::<BendSettingsBoxed>()
                .expect("type checked by GObject");
            *self.settings.lock().unwrap() = boxed.0;
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "settings" => BendSettingsBoxed(self.settings.lock().unwrap().clone()).to_value(),
            name => unreachable!("unknown property {name}"),
        }
    }
}

impl GstObjectImpl for BenderFilter {}

impl ElementImpl for BenderFilter {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static META: OnceLock<gst::subclass::ElementMetadata> = OnceLock::new();
        Some(META.get_or_init(|| {
            gst::subclass::ElementMetadata::new(
                "Bender filter",
                "Filter/Effect/Video",
                "Emulates circuit-bent video hardware",
                "bender-rs",
            )
        }))
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static TEMPLATES: OnceLock<Vec<gst::PadTemplate>> = OnceLock::new();
        TEMPLATES.get_or_init(|| {
            let caps = gstreamer_video::VideoCapsBuilder::new()
                .format_list([
                    VideoFormat::Rgba,
                    VideoFormat::Rgbx,
                    VideoFormat::Bgra,
                    VideoFormat::Bgrx,
                ])
                .build();
            [gst::PadDirection::Src, gst::PadDirection::Sink]
                .into_iter()
                .map(|dir| {
                    let name = if dir == gst::PadDirection::Src {
                        "src"
                    } else {
                        "sink"
                    };
                    gst::PadTemplate::new(name, dir, gst::PadPresence::Always, &caps)
                        .expect("valid pad template")
                })
                .collect()
        })
    }
}

impl BaseTransformImpl for BenderFilter {
    const MODE: BaseTransformMode = BaseTransformMode::NeverInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        self.state.lock().unwrap().reset();
        self.counter.store(0, Ordering::Relaxed);
        Ok(())
    }
}

impl VideoFilterImpl for BenderFilter {
    fn set_info(
        &self,
        _incaps: &gst::Caps,
        in_info: &VideoInfo,
        _outcaps: &gst::Caps,
        _out_info: &VideoInfo,
    ) -> Result<(), gst::LoggableError> {
        *self.info.lock().unwrap() = Some(in_info.clone());
        Ok(())
    }

    fn transform_frame(
        &self,
        in_frame: &VideoFrameRef<&gst::BufferRef>,
        out_frame: &mut VideoFrameRef<&mut gst::BufferRef>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let effect = self.settings.lock().unwrap().clone();
        let fps = self
            .info
            .lock()
            .unwrap()
            .as_ref()
            .map_or(in_frame.info().fps(), |i| i.fps());
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let frame_num = frame_number(in_frame.buffer().pts(), fps).unwrap_or(counter);
        let order = match in_frame.format() {
            VideoFormat::Rgba | VideoFormat::Rgbx => PixelOrder::Rgba,
            VideoFormat::Bgra | VideoFormat::Bgrx => PixelOrder::Bgra,
            _ => return Err(gst::FlowError::NotSupported),
        };
        let width = in_frame.width() as usize;
        let height = in_frame.height() as usize;
        let src_stride = in_frame.plane_stride()[0] as usize;
        let dst_stride = out_frame.plane_stride()[0] as usize;
        let src = in_frame.plane_data(0).map_err(|_| gst::FlowError::Error)?;
        let dst = out_frame
            .plane_data_mut(0)
            .map_err(|_| gst::FlowError::Error)?;
        let mut state = self.state.lock().unwrap();
        effect.apply_rgba8(
            width, height, src, src_stride, dst, dst_stride, order, frame_num, &mut state,
        );
        Ok(gst::FlowSuccess::Ok)
    }
}
