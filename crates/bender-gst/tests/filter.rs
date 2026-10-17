use bender::{BendEffect, InvertMode};
use bender_gst::gst;
use bender_gst::gst::prelude::*;
use bender_gst::{BendSettingsBoxed, BenderFilterElement};
use gstreamer_app::AppSink;

#[test]
fn invert_luma_turns_white_black() {
    bender_gst::init().unwrap();
    let mut effect = BendEffect::default();
    effect.invert.enabled = true;
    effect.invert.settings.mode = InvertMode::InvertLuma;
    effect.invert.settings.mix = 1.0;

    let pipeline = gst::Pipeline::new();
    let src = gst::ElementFactory::make("videotestsrc")
        .property("num-buffers", 2)
        .property_from_str("pattern", "white")
        .build()
        .unwrap();
    let caps = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("format", "RGBA")
                .field("width", 64)
                .field("height", 48)
                .field("framerate", gst::Fraction::new(25, 1))
                .build(),
        )
        .build()
        .unwrap();
    let filter: BenderFilterElement = gst::glib::Object::builder().build();
    filter.set_property("settings", BendSettingsBoxed(effect).to_value());
    let sink = gst::ElementFactory::make("appsink")
        .build()
        .unwrap()
        .downcast::<AppSink>()
        .unwrap();
    let filter = filter.upcast::<gst::Element>();
    pipeline
        .add_many([&src, &caps, &filter, sink.upcast_ref()])
        .unwrap();
    gst::Element::link_many([&src, &caps, &filter, sink.upcast_ref()]).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();
    let sample = sink.pull_sample().unwrap();
    let map = sample.buffer().unwrap().map_readable().unwrap();
    assert_eq!(map.len(), 64 * 48 * 4);
    for px in map.as_chunks::<4>().0 {
        assert!(px[..3].iter().all(|&c| c <= 2), "pixel not black: {px:?}");
    }
    drop(map);
    pipeline.set_state(gst::State::Null).unwrap();
}
