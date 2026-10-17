//! Offline rendering of a file through `benderfilter` to an encoded output.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use bender::BendEffect;
use gst::prelude::*;
use gstreamer as gst;

use crate::decode::build_decode;
use crate::media::{is_still_image, make};
use crate::preview::error_text;
use crate::{BendSettingsBoxed, BenderFilterElement, GstError};

/// Output encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputCodec {
    /// H.264 (x264, constant quantizer 0..=51) + AAC in MP4.
    H264 {
        /// x264 quantizer; lower is higher quality.
        quantizer: u32,
    },
    /// Lossless FFV1 + FLAC in Matroska.
    Ffv1,
    /// Single PNG image (still input only).
    Png,
}

impl OutputCodec {
    /// Codec implied by the output file extension.
    pub fn from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "mp4" => Some(Self::H264 { quantizer: 18 }),
            "mkv" => Some(Self::Ffv1),
            "png" => Some(Self::Png),
            _ => None,
        }
    }

    /// Conventional file extension for this codec.
    pub fn extension(self) -> &'static str {
        match self {
            Self::H264 { .. } => "mp4",
            Self::Ffv1 => "mkv",
            Self::Png => "png",
        }
    }
}

/// Everything needed to render one file.
#[derive(Clone, Debug)]
pub struct RenderSettings {
    /// Input media file.
    pub input: PathBuf,
    /// Output file (overwritten).
    pub output: PathBuf,
    /// Output encoding.
    pub codec: OutputCodec,
    /// Processing/output height in rows; 0 = native.
    pub scanlines: u32,
    /// Effect applied to every frame.
    pub effect: BendEffect,
}

/// Progress of a [`RenderJob`].
#[derive(Clone, Debug, PartialEq)]
pub enum RenderStatus {
    /// Rendering; progress in `0..=1` when the duration is known.
    Running {
        /// Fraction complete.
        progress: Option<f64>,
    },
    /// Finished successfully.
    Done,
    /// Failed with the given message.
    Failed(String),
    /// Cancelled by the caller; partial output deleted.
    Cancelled,
}

/// A running render; dropping it while running cancels it.
pub struct RenderJob {
    pipeline: gst::Pipeline,
    status: Arc<Mutex<RenderStatus>>,
    output: PathBuf,
    thread: Option<JoinHandle<()>>,
}

impl RenderJob {
    /// Validates `settings`, builds the pipeline and starts rendering.
    pub fn start(settings: RenderSettings) -> Result<RenderJob, GstError> {
        let still = is_still_image(&settings.input);
        match (still, settings.codec) {
            (true, OutputCodec::Png) | (false, OutputCodec::H264 { .. } | OutputCodec::Ffv1) => {}
            (true, _) => {
                return Err(GstError::Unsupported(
                    "still images can only be rendered to .png".into(),
                ));
            }
            (false, OutputCodec::Png) => {
                return Err(GstError::Unsupported(
                    "png output requires a still image input".into(),
                ));
            }
        }

        let pipeline = gst::Pipeline::new();
        let filesink = make("filesink")?;
        filesink.set_property("location", settings.output.to_string_lossy().as_ref());
        pipeline.add(&filesink)?;
        let mux = match settings.codec {
            OutputCodec::H264 { .. } => Some(make("mp4mux")?),
            OutputCodec::Ffv1 => Some(make("matroskamux")?),
            OutputCodec::Png => None,
        };
        if let Some(mux) = &mux {
            pipeline.add(mux)?;
            mux.link(&filesink)?;
        }

        let codec = settings.codec;
        let effect = settings.effect;
        let video_mux = mux.clone();
        let video_sink = filesink.clone();
        let audio_mux = mux;
        build_decode(
            &pipeline,
            &settings.input,
            settings.scanlines,
            None,
            move |pipeline, last| {
                let filter: BenderFilterElement = gst::glib::Object::builder().build();
                filter.set_property("settings", BendSettingsBoxed(effect.clone()).to_value());
                let mut chain: Vec<gst::Element> = vec![filter.upcast(), make("videoconvert")?];
                match codec {
                    OutputCodec::H264 { quantizer } => {
                        let caps = make("capsfilter")?;
                        caps.set_property(
                            "caps",
                            gst::Caps::builder("video/x-raw")
                                .field("format", "I420")
                                .build(),
                        );
                        let enc = make("x264enc")?;
                        enc.set_property_from_str("pass", "quant");
                        enc.set_property("quantizer", quantizer.min(51));
                        enc.set_property_from_str("speed-preset", "medium");
                        chain.extend([caps, enc]);
                    }
                    OutputCodec::Ffv1 => chain.push(make("avenc_ffv1")?),
                    OutputCodec::Png => {
                        let enc = make("pngenc")?;
                        enc.set_property("snapshot", true);
                        chain.push(enc);
                    }
                }
                pipeline.add_many(&chain)?;
                gst::Element::link_many(&chain)?;
                let tail = chain.last().expect("non-empty");
                tail.link(video_mux.as_ref().unwrap_or(&video_sink))?;
                for e in chain.iter().rev() {
                    e.sync_state_with_parent()?;
                }
                last.link(&chain[0])?;
                Ok(())
            },
            move |pipeline, last| {
                let Some(mux) = &audio_mux else {
                    return Ok(());
                };
                let enc = match codec {
                    OutputCodec::H264 { .. } => make("avenc_aac")?,
                    _ => make("flacenc")?,
                };
                pipeline.add(&enc)?;
                enc.link(mux)?;
                enc.sync_state_with_parent()?;
                last.link(&enc)?;
                Ok(())
            },
        )?;

        let status = Arc::new(Mutex::new(RenderStatus::Running { progress: None }));
        if let Err(e) = pipeline.set_state(gst::State::Playing) {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(e.into());
        }
        let bus = pipeline
            .bus()
            .ok_or_else(|| GstError::Pipeline("pipeline has no bus".into()))?;
        let thread_pipeline = pipeline.clone();
        let thread_status = status.clone();
        let thread = std::thread::spawn(move || {
            watch_bus(&bus, &thread_pipeline, &thread_status);
        });
        Ok(RenderJob {
            pipeline,
            status,
            output: settings.output,
            thread: Some(thread),
        })
    }

    /// Current status.
    pub fn status(&self) -> RenderStatus {
        self.status.lock().unwrap().clone()
    }

    /// Stops a running render and deletes the partial output.
    pub fn cancel(&self) {
        let mut status = self.status.lock().unwrap();
        if matches!(*status, RenderStatus::Running { .. }) {
            let _ = self.pipeline.set_state(gst::State::Null);
            let _ = std::fs::remove_file(&self.output);
            *status = RenderStatus::Cancelled;
        }
    }
}

impl Drop for RenderJob {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn watch_bus(bus: &gst::Bus, pipeline: &gst::Pipeline, status: &Mutex<RenderStatus>) {
    loop {
        let msg = bus.timed_pop_filtered(
            gst::ClockTime::from_mseconds(250),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        );
        let mut status = status.lock().unwrap();
        if !matches!(*status, RenderStatus::Running { .. }) {
            return;
        }
        let finished = match msg.as_ref().map(|m| m.view()) {
            Some(gst::MessageView::Eos(_)) => Some(RenderStatus::Done),
            Some(gst::MessageView::Error(err)) => Some(RenderStatus::Failed(error_text(err))),
            _ => None,
        };
        if let Some(done) = finished {
            let _ = pipeline.set_state(gst::State::Null);
            *status = done;
            return;
        }
        let progress = match (
            pipeline.query_position::<gst::ClockTime>(),
            pipeline.query_duration::<gst::ClockTime>(),
        ) {
            (Some(pos), Some(dur)) if dur.nseconds() > 0 => {
                Some((pos.nseconds() as f64 / dur.nseconds() as f64).clamp(0.0, 1.0))
            }
            _ => None,
        };
        *status = RenderStatus::Running { progress };
    }
}
