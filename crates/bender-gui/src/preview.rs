//! Frame state shared between the GStreamer streaming thread and the UI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

use bender::{BendEffect, BendState, PixelOrder};
use bender_gst::PreviewFrame;
use eframe::egui;

/// Latest decoded frame plus the effect applied to it, re-rendered on demand.
pub struct PreviewShared {
    pub settings: RwLock<BendEffect>,
    pub bypass: AtomicBool,
    frame: Mutex<Option<PreviewFrame>>,
    state: Mutex<BendState>,
    texture: Mutex<Option<egui::TextureHandle>>,
    ctx: egui::Context,
}

impl PreviewShared {
    pub fn new(ctx: egui::Context, settings: BendEffect) -> Self {
        Self {
            settings: RwLock::new(settings),
            bypass: AtomicBool::new(false),
            frame: Mutex::new(None),
            state: Mutex::new(BendState::default()),
            texture: Mutex::new(None),
            ctx,
        }
    }

    /// Appsink callback: stores the frame and renders it.
    pub fn on_frame(&self, frame: PreviewFrame) {
        *self.frame.lock().unwrap() = Some(frame);
        self.render();
    }

    /// Clears the stored frame, texture and temporal effect state.
    pub fn clear(&self) {
        *self.frame.lock().unwrap() = None;
        *self.texture.lock().unwrap() = None;
        self.state.lock().unwrap().reset();
    }

    /// Current preview texture, if a frame has been rendered.
    pub fn texture(&self) -> Option<egui::TextureHandle> {
        self.texture.lock().unwrap().clone()
    }

    /// Re-applies the effect to the last frame and uploads it as the preview texture.
    pub fn render(&self) {
        let frame = self.frame.lock().unwrap();
        let Some(f) = frame.as_ref() else {
            return;
        };
        let mut image = egui::ColorImage::filled([f.width, f.height], egui::Color32::BLACK);
        let dst = image.as_raw_mut();
        if self.bypass.load(Ordering::Relaxed) {
            let row = f.width * 4;
            for (d, s) in dst.chunks_exact_mut(row).zip(f.data.chunks(f.stride)) {
                d.copy_from_slice(&s[..row]);
            }
        } else {
            let mut state = self.state.lock().unwrap();
            self.settings.read().unwrap().apply_rgba8(
                f.width,
                f.height,
                &f.data,
                f.stride,
                dst,
                f.width * 4,
                PixelOrder::Rgba,
                f.frame_num,
                &mut state,
            );
        }
        drop(frame);
        let options = egui::TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
            ..Default::default()
        };
        let mut texture = self.texture.lock().unwrap();
        match texture.as_mut() {
            Some(t) => t.set(image, options),
            None => *texture = Some(self.ctx.load_texture("preview", image, options)),
        }
        self.ctx.request_repaint();
    }
}
