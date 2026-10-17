//! Interactive preview pipeline delivering decoded RGBA frames to a callback.

use std::path::Path;
use std::sync::Arc;

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app::{AppSink, AppSinkCallbacks};
use gstreamer_video::VideoInfo;

use crate::GstError;
use crate::decode::build_decode;
use crate::media::{frame_number, is_still_image, make};

/// One decoded (unfiltered) RGBA frame.
#[derive(Clone, Debug)]
pub struct PreviewFrame {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Bytes per row of `data`.
    pub stride: usize,
    /// RGBA pixel bytes.
    pub data: Vec<u8>,
    /// Frame index derived from the timestamp (0 if unknown).
    pub frame_num: u64,
}

/// A playing/paused preview of a media file; dropped pipelines go to `Null`.
pub struct PreviewPipeline {
    /// The underlying pipeline.
    pub pipeline: gst::Pipeline,
    still: bool,
}

impl PreviewPipeline {
    /// Builds the preview for `path` scaled to `scanlines` (0 = native) and
    /// sets it to `Paused`; `on_frame` runs on the streaming thread.
    pub fn new(
        path: &Path,
        scanlines: u32,
        on_frame: impl Fn(PreviewFrame) + Send + Sync + 'static,
    ) -> Result<Self, GstError> {
        let still = is_still_image(path);
        let pipeline = gst::Pipeline::new();
        let on_frame = Arc::new(on_frame);
        build_decode(
            &pipeline,
            path,
            scanlines,
            still.then(|| gst::Fraction::new(30, 1)),
            move |pipeline, last| {
                let sink = make("appsink")?
                    .downcast::<AppSink>()
                    .map_err(|_| GstError::Pipeline("appsink has unexpected type".into()))?;
                sink.set_sync(true);
                sink.set_max_buffers(1);
                sink.set_drop(true);
                sink.set_caps(Some(
                    &gst::Caps::builder("video/x-raw")
                        .field("format", "RGBA")
                        .build(),
                ));
                let on_sample = on_frame.clone();
                let on_preroll = on_frame.clone();
                sink.set_callbacks(
                    AppSinkCallbacks::builder()
                        .new_sample(move |s| {
                            let sample = s.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                            deliver(&sample, &*on_sample)
                        })
                        .new_preroll(move |s| {
                            let sample = s.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                            deliver(&sample, &*on_preroll)
                        })
                        .build(),
                );
                pipeline.add(&sink)?;
                sink.sync_state_with_parent()?;
                last.link(&sink)?;
                Ok(())
            },
            |pipeline, last| {
                let volume = make("volume")?;
                volume.set_property("name", "preview_volume");
                let sink = make("autoaudiosink")?;
                pipeline.add_many([&volume, &sink])?;
                volume.sync_state_with_parent()?;
                sink.sync_state_with_parent()?;
                gst::Element::link_many([last, &volume, &sink])?;
                Ok(())
            },
        )?;
        pipeline.set_state(gst::State::Paused)?;
        Ok(Self { pipeline, still })
    }

    /// Starts playback.
    pub fn play(&self) -> Result<(), GstError> {
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(())
    }

    /// Pauses playback.
    pub fn pause(&self) -> Result<(), GstError> {
        self.pipeline.set_state(gst::State::Paused)?;
        Ok(())
    }

    /// Accurately seeks to `pos` (flushing).
    pub fn seek(&self, pos: gst::ClockTime) -> Result<(), GstError> {
        self.pipeline
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, pos)?;
        Ok(())
    }

    /// Current playback position.
    pub fn position(&self) -> Option<gst::ClockTime> {
        self.pipeline.query_position::<gst::ClockTime>()
    }

    /// Stream duration, if known.
    pub fn duration(&self) -> Option<gst::ClockTime> {
        self.pipeline.query_duration::<gst::ClockTime>()
    }

    /// Sets audio volume (1.0 = unity) if the input has audio.
    pub fn set_volume(&self, v: f64) {
        if let Some(volume) = self.pipeline.by_name("preview_volume") {
            volume.set_property("volume", v.clamp(0.0, 10.0));
        }
    }

    /// Whether the input is a still image.
    pub fn is_still(&self) -> bool {
        self.still
    }

    /// Drains pending bus messages without blocking: loops to the start on
    /// EOS and returns the first error, if any.
    pub fn poll_error(&self) -> Option<String> {
        let bus = self.pipeline.bus()?;
        let mut error = None;
        while let Some(msg) = bus.pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos]) {
            match msg.view() {
                gst::MessageView::Eos(_) => {
                    if let Err(e) = self.seek(gst::ClockTime::ZERO) {
                        error.get_or_insert_with(|| e.to_string());
                    }
                }
                gst::MessageView::Error(err) => {
                    error.get_or_insert_with(|| error_text(err));
                }
                _ => {}
            }
        }
        error
    }
}

impl Drop for PreviewPipeline {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

/// Formats a bus error with its debug details.
pub(crate) fn error_text(err: &gst::message::Error) -> String {
    match err.debug() {
        Some(debug) => format!("{} ({debug})", err.error()),
        None => err.error().to_string(),
    }
}

fn deliver(
    sample: &gst::Sample,
    on_frame: &(impl Fn(PreviewFrame) + ?Sized),
) -> Result<gst::FlowSuccess, gst::FlowError> {
    let caps = sample.caps().ok_or(gst::FlowError::NotNegotiated)?;
    let info = VideoInfo::from_caps(caps).map_err(|_| gst::FlowError::NotNegotiated)?;
    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
    on_frame(PreviewFrame {
        width: info.width() as usize,
        height: info.height() as usize,
        stride: info.stride()[0] as usize,
        data: map.as_slice().to_vec(),
        frame_num: frame_number(buffer.pts(), info.fps()).unwrap_or(0),
    });
    Ok(gst::FlowSuccess::Ok)
}
