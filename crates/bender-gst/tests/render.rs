use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bender::BendEffect;
use bender_gst::gst;
use bender_gst::gst::prelude::*;
use bender_gst::{OutputCodec, RenderJob, RenderSettings, RenderStatus};
use gstreamer_app::AppSink;

const FRAMES: usize = 12;

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("bender-render-{}-{name}", std::process::id()))
}

fn write_input(path: &Path) {
    let pipeline = gst::parse::launch(&format!(
        "videotestsrc num-buffers={FRAMES} pattern=smpte \
         ! video/x-raw,width=96,height=64,framerate=25/1 \
         ! avenc_ffv1 ! matroskamux ! filesink location=\"{}\"",
        path.display()
    ))
    .unwrap();
    run_to_eos(&pipeline);
}

fn run_to_eos(pipeline: &gst::Element) {
    pipeline.set_state(gst::State::Playing).unwrap();
    let bus = pipeline.bus().unwrap();
    let msg = bus
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(30),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("pipeline timed out");
    pipeline.set_state(gst::State::Null).unwrap();
    if let gst::MessageView::Error(err) = msg.view() {
        panic!("pipeline error: {}", err.error());
    }
}

fn decoded_frames(path: &Path) -> usize {
    let pipeline = gst::parse::launch(&format!(
        "filesrc location=\"{}\" ! decodebin ! videoconvert ! appsink name=sink sync=false",
        path.display()
    ))
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();
    let sink = pipeline
        .by_name("sink")
        .unwrap()
        .downcast::<AppSink>()
        .unwrap();
    pipeline.set_state(gst::State::Playing).unwrap();
    let mut frames = 0;
    while sink.pull_sample().is_ok() {
        frames += 1;
    }
    pipeline.set_state(gst::State::Null).unwrap();
    frames
}

fn render(input: &Path, codec: OutputCodec) -> PathBuf {
    let output = temp_path(&format!("out.{}", codec.extension()));
    let mut effect = BendEffect::default();
    effect.invert.enabled = true;
    let job = RenderJob::start(RenderSettings {
        input: input.to_owned(),
        output: output.clone(),
        codec,
        scanlines: 0,
        effect,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match job.status() {
            RenderStatus::Running { .. } => {
                assert!(Instant::now() < deadline, "render timed out");
                std::thread::sleep(Duration::from_millis(20));
            }
            RenderStatus::Done => break,
            other => panic!("render ended with {other:?}"),
        }
    }
    output
}

#[test]
fn video_renders_keep_every_frame() {
    bender_gst::init().unwrap();
    let input = temp_path("in.mkv");
    write_input(&input);
    for codec in [OutputCodec::Ffv1, OutputCodec::H264 { quantizer: 18 }] {
        let output = render(&input, codec);
        assert_eq!(decoded_frames(&output), FRAMES, "{codec:?}");
        std::fs::remove_file(&output).unwrap();
    }
    std::fs::remove_file(&input).unwrap();
}
