//! Shared `filesrc ! decodebin` front-end with scaled RGBA video and converted audio.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use gst::prelude::*;
use gstreamer as gst;

use crate::GstError;
use crate::media::{make, scaled_size};

/// Adds a decoder for `path` to `pipeline`.
///
/// The first video stream is scaled to `scanlines` rows (`0` = native) as RGBA
/// and handed to `on_video` as the last element of the branch; with
/// `still_framerate` it is frozen into a stream at that rate. The first audio
/// stream ends in an `audioresample` handed to `on_audio`.
pub fn build_decode(
    pipeline: &gst::Pipeline,
    path: &Path,
    scanlines: u32,
    still_framerate: Option<gst::Fraction>,
    on_video: impl Fn(&gst::Pipeline, &gst::Element) -> Result<(), GstError> + Send + Sync + 'static,
    on_audio: impl Fn(&gst::Pipeline, &gst::Element) -> Result<(), GstError> + Send + Sync + 'static,
) -> Result<(), GstError> {
    let src = make("filesrc")?;
    src.set_property("location", path.to_string_lossy().as_ref());
    let decodebin = make("decodebin")?;
    pipeline.add_many([&src, &decodebin])?;
    src.link(&decodebin)?;

    let weak = pipeline.downgrade();
    let video_done = AtomicBool::new(false);
    let audio_done = AtomicBool::new(false);
    decodebin.connect_pad_added(move |dbin, pad| {
        let Some(pipeline) = weak.upgrade() else {
            return;
        };
        let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
        let Some(s) = caps.structure(0) else { return };
        let result = if s.name().starts_with("video/") {
            if video_done.swap(true, Ordering::SeqCst) {
                return;
            }
            let w = s.get::<i32>("width").unwrap_or(640);
            let h = s.get::<i32>("height").unwrap_or(480);
            link_video(&pipeline, pad, w, h, scanlines, still_framerate)
                .and_then(|last| on_video(&pipeline, &last))
        } else if s.name().starts_with("audio/") {
            if audio_done.swap(true, Ordering::SeqCst) {
                return;
            }
            link_chain(&pipeline, pad, &["queue", "audioconvert", "audioresample"])
                .and_then(|last| on_audio(&pipeline, &last))
        } else {
            return;
        };
        if let Err(e) = result {
            gst::element_error!(dbin, gst::CoreError::Negotiation, ["{e}"]);
        }
    });
    Ok(())
}

fn link_video(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    w: i32,
    h: i32,
    scanlines: u32,
    still_framerate: Option<gst::Fraction>,
) -> Result<gst::Element, GstError> {
    let mut names = vec!["queue", "videoconvert", "videoscale"];
    if still_framerate.is_some() {
        names.push("imagefreeze");
    }
    names.push("capsfilter");
    let (ow, oh) = scaled_size(w, h, scanlines);
    let mut caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("width", ow)
        .field("height", oh)
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1));
    if let Some(fr) = still_framerate {
        caps = caps.field("framerate", fr);
    }
    let caps = caps.build();
    let elements = names
        .iter()
        .map(|n| make(n))
        .collect::<Result<Vec<_>, _>>()?;
    let capsfilter = elements.last().expect("non-empty").clone();
    capsfilter.set_property("caps", &caps);
    add_and_link(pipeline, pad, &elements)?;
    Ok(capsfilter)
}

fn link_chain(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    names: &[&str],
) -> Result<gst::Element, GstError> {
    let elements = names
        .iter()
        .map(|n| make(n))
        .collect::<Result<Vec<_>, _>>()?;
    add_and_link(pipeline, pad, &elements)?;
    Ok(elements.last().expect("non-empty").clone())
}

fn add_and_link(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    elements: &[gst::Element],
) -> Result<(), GstError> {
    pipeline.add_many(elements)?;
    gst::Element::link_many(elements)?;
    for e in elements {
        e.sync_state_with_parent()?;
    }
    let sink = elements[0]
        .static_pad("sink")
        .ok_or_else(|| GstError::Pipeline("queue has no sink pad".into()))?;
    pad.link(&sink)
        .map_err(|e| GstError::Pipeline(format!("failed to link decoder pad: {e:?}")))?;
    Ok(())
}
