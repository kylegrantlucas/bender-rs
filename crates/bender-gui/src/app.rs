//! Main window: settings, preview, transport and render queue.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bender::{BendEffect, SettingKind, SettingValue, setting_groups};
use bender_gst::gst::ClockTime;
use bender_gst::{OutputCodec, PreviewPipeline, RenderJob, RenderSettings, RenderStatus};
use eframe::egui;

use crate::preview::PreviewShared;

const SETTINGS_KEY: &str = "bender.settings";
const SCANLINES_KEY: &str = "bender.scanlines";
const DEFAULT_QUANTIZER: u32 = 18;

/// The bender-rs standalone application.
pub struct BenderApp {
    shared: Arc<PreviewShared>,
    pipeline: Option<PreviewPipeline>,
    input: Option<PathBuf>,
    scanlines: u32,
    playing: bool,
    error: Option<String>,
    render_codec: OutputCodec,
    render_quantizer: u32,
    jobs: Vec<(PathBuf, RenderJob)>,
    /// Seek slider position (seconds) while the user is dragging it.
    seek_drag: Option<f64>,
}

impl BenderApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        let mut settings = BendEffect::default();
        let mut scanlines = 480;
        let mut error = None;
        if let Some(storage) = cc.storage {
            if let Some(json) = eframe::get_value::<String>(storage, SETTINGS_KEY) {
                match BendEffect::from_json(&json) {
                    Ok(e) => settings = e,
                    Err(e) => error = Some(format!("restoring settings: {e}")),
                }
            }
            if let Some(s) = eframe::get_value::<u32>(storage, SCANLINES_KEY) {
                scanlines = s;
            }
        }
        let mut app = Self {
            shared: Arc::new(PreviewShared::new(cc.egui_ctx.clone(), settings)),
            pipeline: None,
            input: None,
            scanlines,
            playing: false,
            error,
            render_codec: OutputCodec::H264 {
                quantizer: DEFAULT_QUANTIZER,
            },
            render_quantizer: DEFAULT_QUANTIZER,
            jobs: Vec::new(),
            seek_drag: None,
        };
        if let Some(path) = open {
            app.open(path);
        }
        app
    }

    /// Opens `path` and starts playing it.
    fn open(&mut self, path: PathBuf) {
        self.input = Some(path);
        self.rebuild(None, true);
    }

    /// (Re)builds the preview pipeline for the current input, optionally seeking to `position`.
    fn rebuild(&mut self, position: Option<ClockTime>, play: bool) {
        self.pipeline = None;
        self.shared.clear();
        self.playing = false;
        let Some(path) = self.input.clone() else {
            return;
        };
        let shared = Arc::clone(&self.shared);
        let pipeline =
            match PreviewPipeline::new(&path, self.scanlines, move |f| shared.on_frame(f)) {
                Ok(p) => p,
                Err(e) => {
                    self.error = Some(format!("opening {}: {e}", path.display()));
                    return;
                }
            };
        let result = pipeline.pause().and_then(|()| match position {
            Some(pos) if !pos.is_zero() => pipeline.seek(pos),
            _ => Ok(()),
        });
        let result = result.and_then(|()| if play { pipeline.play() } else { Ok(()) });
        match result {
            Ok(()) => self.playing = play,
            Err(e) => self.error = Some(e.to_string()),
        }
        self.pipeline = Some(pipeline);
    }

    fn set_playing(&mut self, play: bool) {
        let Some(pipeline) = &self.pipeline else {
            return;
        };
        let result = if play {
            pipeline.play()
        } else {
            pipeline.pause()
        };
        match result {
            Ok(()) => self.playing = play,
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn top_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open…").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_file()
            {
                self.open(path);
            }
            if ui.button("Load preset…").clicked()
                && let Some(path) = json_dialog().pick_file()
            {
                match std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|s| BendEffect::from_json(&s).map_err(|e| e.to_string()))
                {
                    Ok(effect) => self.set_effect(effect),
                    Err(e) => self.error = Some(format!("loading {}: {e}", path.display())),
                }
            }
            if ui.button("Save preset…").clicked()
                && let Some(path) = json_dialog().set_file_name("preset.json").save_file()
            {
                let json = self.shared.settings.read().unwrap().to_json();
                if let Err(e) = std::fs::write(&path, json) {
                    self.error = Some(format!("saving {}: {e}", path.display()));
                }
            }
            if ui.button("Reset").clicked() {
                self.set_effect(BendEffect::default());
            }
            if let Some(err) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, err);
                if ui.small_button("×").clicked() {
                    self.error = None;
                }
            }
        });
    }

    fn set_effect(&self, effect: BendEffect) {
        *self.shared.settings.write().unwrap() = effect;
        self.shared.render();
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        let mut effect = self.shared.settings.read().unwrap().clone();
        ui.horizontal(|ui| {
            ui.label("Seed");
            ui.add(egui::DragValue::new(&mut effect.seed));
            if ui.button("Randomize").clicked() {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.subsec_nanos() as u64 ^ d.as_secs());
                effect.seed = (nanos & 0x7fff_ffff) as i32;
            }
        });
        ui.separator();
        for group in setting_groups() {
            let header = egui::CollapsingHeader::new(group.label).id_salt(group.id);
            header
                .show(ui, |ui| {
                    for desc in &group.settings {
                        setting_widget(ui, desc, &mut effect).on_hover_text(desc.description);
                    }
                })
                .header_response
                .on_hover_text(group.description);
        }
        if effect != *self.shared.settings.read().unwrap() {
            self.set_effect(effect);
        }
    }

    fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let has_pipeline = self.pipeline.is_some();
            let label = if self.playing { "⏸" } else { "▶" };
            if ui
                .add_enabled(has_pipeline, egui::Button::new(label))
                .clicked()
            {
                self.set_playing(!self.playing);
            }

            let (position, duration) = self
                .pipeline
                .as_ref()
                .map_or((None, None), |p| (p.position(), p.duration()));
            let dur = duration.map_or(0.0, seconds);
            let pos = self
                .seek_drag
                .unwrap_or_else(|| position.map_or(0.0, seconds));
            let mut value = pos;
            let response = ui.add_enabled(
                dur > 0.0,
                egui::Slider::new(&mut value, 0.0..=dur.max(f64::EPSILON)).show_value(false),
            );
            if response.dragged() || response.changed() {
                self.seek_drag = Some(value);
            }
            if (response.drag_stopped() || (response.changed() && !response.dragged()))
                && let (Some(target), Some(p)) = (self.seek_drag.take(), &self.pipeline)
                && let Err(e) = p.seek(ClockTime::from_nseconds((target * 1e9) as u64))
            {
                self.error = Some(e.to_string());
            }
            ui.label(format!("{} / {}", timestamp(value), timestamp(dur)));

            let mut bypass = self.shared.bypass.load(Ordering::Relaxed);
            if ui.checkbox(&mut bypass, "Bypass").changed() {
                self.shared.bypass.store(bypass, Ordering::Relaxed);
                self.shared.render();
            }

            ui.label("Scanlines");
            let mut scanlines = self.scanlines;
            ui.add(egui::DragValue::new(&mut scanlines).range(0..=2160))
                .on_hover_text("Processing height in rows; 0 = native resolution");
            if scanlines != self.scanlines {
                self.scanlines = scanlines;
                let playing = self.playing;
                self.rebuild(position, playing);
            }
        });
    }

    fn render_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Render");
        egui::ComboBox::from_label("Codec")
            .selected_text(codec_label(self.render_codec))
            .show_ui(ui, |ui| {
                for codec in [
                    OutputCodec::H264 {
                        quantizer: self.render_quantizer,
                    },
                    OutputCodec::Ffv1,
                    OutputCodec::Png,
                ] {
                    let selected = std::mem::discriminant(&codec)
                        == std::mem::discriminant(&self.render_codec);
                    if ui.selectable_label(selected, codec_label(codec)).clicked() {
                        self.render_codec = codec;
                    }
                }
            });
        if matches!(self.render_codec, OutputCodec::H264 { .. }) {
            ui.add(egui::Slider::new(&mut self.render_quantizer, 0..=51).text("Quantizer"))
                .on_hover_text("x264 constant quantizer; lower = higher quality");
            self.render_codec = OutputCodec::H264 {
                quantizer: self.render_quantizer,
            };
        }
        if ui
            .add_enabled(self.input.is_some(), egui::Button::new("Render…"))
            .clicked()
        {
            self.start_render();
        }
        ui.separator();
        let mut running = false;
        for (path, job) in &self.jobs {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into(),
            );
            ui.label(name);
            match job.status() {
                RenderStatus::Running { progress } => {
                    running = true;
                    ui.horizontal(|ui| {
                        let bar = match progress {
                            Some(p) => egui::ProgressBar::new(p as f32).show_percentage(),
                            None => egui::ProgressBar::new(0.0).animate(true),
                        };
                        ui.add(bar.desired_width(140.0));
                        if ui.button("Cancel").clicked() {
                            job.cancel();
                        }
                    });
                }
                RenderStatus::Done => {
                    ui.label("Done");
                }
                RenderStatus::Failed(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Failed: {e}"));
                }
                RenderStatus::Cancelled => {
                    ui.label("Cancelled");
                }
            }
            ui.add_space(4.0);
        }
        if running {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
    }

    fn start_render(&mut self) {
        let Some(input) = self.input.clone() else {
            return;
        };
        let ext = self.render_codec.extension();
        let stem = input
            .file_stem()
            .map_or_else(|| "output".into(), |s| s.to_string_lossy().into_owned());
        let Some(mut output) = rfd::FileDialog::new()
            .add_filter(ext, &[ext])
            .set_file_name(format!("{stem}-bent.{ext}"))
            .save_file()
        else {
            return;
        };
        if output.extension().is_none() {
            output.set_extension(ext);
        }
        let settings = RenderSettings {
            input,
            output: output.clone(),
            codec: self.render_codec,
            scanlines: self.scanlines,
            effect: self.shared.settings.read().unwrap().clone(),
        };
        match RenderJob::start(settings) {
            Ok(job) => self.jobs.push((output, job)),
            Err(e) => self.error = Some(format!("render: {e}")),
        }
    }

    fn central_panel(&self, ui: &mut egui::Ui) {
        match self.shared.texture() {
            Some(texture) => {
                let avail = ui.available_size();
                let size = texture.size_vec2();
                let scale = (avail.x / size.x).min(avail.y / size.y);
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Image::new(&texture).fit_to_exact_size(size * scale));
                });
            }
            None => {
                ui.centered_and_justified(|ui| ui.label("Open a video or image"));
            }
        }
    }
}

impl eframe::App for BenderApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(err) = self.pipeline.as_ref().and_then(PreviewPipeline::poll_error) {
            self.error = Some(err);
        }
        egui::Panel::top("top").show(ui, |ui| self.top_panel(ui));
        egui::Panel::bottom("transport").show(ui, |ui| self.bottom_panel(ui));
        egui::Panel::left("settings")
            .resizable(true)
            .default_size(320.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.settings_panel(ui));
            });
        egui::Panel::right("render")
            .resizable(true)
            .default_size(240.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.render_panel(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| self.central_panel(ui));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let json = self.shared.settings.read().unwrap().to_json();
        eframe::set_value(storage, SETTINGS_KEY, &json);
        eframe::set_value(storage, SCANLINES_KEY, &self.scanlines);
    }
}

/// Draws the widget for one setting descriptor, applying edits to `effect`.
fn setting_widget(
    ui: &mut egui::Ui,
    desc: &bender::SettingDescriptor,
    effect: &mut BendEffect,
) -> egui::Response {
    match (desc.kind, (desc.get)(effect)) {
        (
            SettingKind::Float {
                min,
                max,
                logarithmic,
            },
            SettingValue::Float(mut v),
        ) => {
            let r = ui.add(
                egui::Slider::new(&mut v, min..=max)
                    .logarithmic(logarithmic)
                    .text(desc.label),
            );
            (desc.set)(effect, SettingValue::Float(v));
            r
        }
        (SettingKind::Int { min, max }, SettingValue::Int(mut v)) => {
            let r = ui.add(egui::Slider::new(&mut v, min..=max).text(desc.label));
            (desc.set)(effect, SettingValue::Int(v));
            r
        }
        (SettingKind::Bool, SettingValue::Bool(mut v)) => {
            let r = ui.checkbox(&mut v, desc.label);
            (desc.set)(effect, SettingValue::Bool(v));
            r
        }
        (SettingKind::Enum { options }, SettingValue::Enum(mut v)) => {
            let selected = options.get(v as usize).map_or("?", |o| o.1);
            let r = egui::ComboBox::from_label(desc.label)
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (i, (_, label)) in options.iter().enumerate() {
                        ui.selectable_value(&mut v, i as u32, *label);
                    }
                })
                .response;
            (desc.set)(effect, SettingValue::Enum(v));
            r
        }
        _ => ui.label(desc.label),
    }
}

fn json_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("JSON preset", &["json"])
}

fn codec_label(codec: OutputCodec) -> &'static str {
    match codec {
        OutputCodec::H264 { .. } => "H.264 MP4",
        OutputCodec::Ffv1 => "FFV1 MKV",
        OutputCodec::Png => "PNG",
    }
}

fn seconds(t: ClockTime) -> f64 {
    t.nseconds() as f64 / 1e9
}

/// Formats seconds as `mm:ss.ff`.
fn timestamp(secs: f64) -> String {
    let centis = (secs.max(0.0) * 100.0).round() as u64;
    format!(
        "{:02}:{:02}.{:02}",
        centis / 6000,
        centis / 100 % 60,
        centis % 100
    )
}
