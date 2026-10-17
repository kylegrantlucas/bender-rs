//! C-ABI exports of the bender effect for `wasm32-unknown-unknown`.
//!
//! No bindings generator: all buffers are Rust-owned and JavaScript reads and
//! writes them through views on the module's linear memory.
//!
//! 1. `bender_frame_buffer(w, h)` returns the input RGBA buffer; JS fills it.
//! 2. `bender_process(w, h, frame)` renders it; `bender_output()` points at the
//!    result (same size, RGBA).
//! 3. Text crosses the boundary through one shared byte buffer: JS writes UTF-8
//!    into `bender_text_buffer(len)` before `bender_set_preset(len)`; Rust writes
//!    replies there and returns their length (`bender_schema`, errors).

use std::cell::RefCell;

use bender::{BendEffect, BendState, PixelOrder, SettingKind, SettingValue, setting_groups};
use serde_json::{Value, json};

#[derive(Default)]
struct Engine {
    effect: BendEffect,
    state: BendState,
    input: Vec<u8>,
    output: Vec<u8>,
    text: Vec<u8>,
}

thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::default());
}

fn with<R>(f: impl FnOnce(&mut Engine) -> R) -> R {
    ENGINE.with(|e| f(&mut e.borrow_mut()))
}

fn rgba_len(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

/// Input buffer for a `width`×`height` RGBA frame (reallocated on size change).
#[unsafe(no_mangle)]
pub extern "C" fn bender_frame_buffer(width: u32, height: u32) -> *mut u8 {
    with(|e| {
        let len = rgba_len(width, height);
        e.input.resize(len, 0);
        e.output.resize(len, 0);
        e.input.as_mut_ptr()
    })
}

/// Rendered RGBA output of the last [`bender_process`].
#[unsafe(no_mangle)]
pub extern "C" fn bender_output() -> *const u8 {
    with(|e| e.output.as_ptr())
}

/// Applies the current effect to the input buffer. Returns 0, or 1 if the
/// buffers were not sized for `width`×`height` by [`bender_frame_buffer`].
#[unsafe(no_mangle)]
pub extern "C" fn bender_process(width: u32, height: u32, frame: f64) -> i32 {
    with(|e| {
        let len = rgba_len(width, height);
        if e.input.len() != len || e.output.len() != len {
            return 1;
        }
        let (w, h) = (width as usize, height as usize);
        let Engine {
            effect,
            state,
            input,
            output,
            ..
        } = e;
        effect.apply_rgba8(
            w,
            h,
            input,
            w * 4,
            output,
            w * 4,
            PixelOrder::Rgba,
            frame.max(0.0) as u64,
            state,
        );
        0
    })
}

/// Forgets temporal state (frame-store memory), e.g. after switching sources.
#[unsafe(no_mangle)]
pub extern "C" fn bender_reset() {
    with(|e| e.state.reset());
}

/// Shared text buffer, resized to `len` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn bender_text_buffer(len: u32) -> *mut u8 {
    with(|e| {
        e.text.resize(len as usize, 0);
        e.text.as_mut_ptr()
    })
}

/// Pointer to the shared text buffer (valid until the next call that writes text).
#[unsafe(no_mangle)]
pub extern "C" fn bender_text_ptr() -> *const u8 {
    with(|e| e.text.as_ptr())
}

fn write_text(e: &mut Engine, s: &str) -> i32 {
    e.text.clear();
    e.text.extend_from_slice(s.as_bytes());
    i32::try_from(e.text.len()).unwrap_or(i32::MAX)
}

/// Loads preset JSON from the first `len` bytes of the text buffer.
///
/// Returns 0 on success; otherwise a negative length `-n`, with the `n`-byte
/// error message in the text buffer.
#[unsafe(no_mangle)]
pub extern "C" fn bender_set_preset(len: u32) -> i32 {
    with(|e| {
        let parsed = std::str::from_utf8(&e.text[..(len as usize).min(e.text.len())])
            .map_err(|err| err.to_string())
            .and_then(|s| BendEffect::from_json(s).map_err(|err| err.to_string()));
        match parsed {
            Ok(effect) => {
                e.effect = effect;
                0
            }
            Err(msg) => -write_text(e, &msg),
        }
    })
}

/// Writes the current (clamped) preset JSON to the text buffer; returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn bender_get_preset() -> i32 {
    with(|e| {
        let s = e.effect.to_json();
        write_text(e, &s)
    })
}

/// Writes the settings schema JSON to the text buffer; returns its length.
///
/// `{ "version", "defaults": <preset>, "groups": [{ id, label, description,
/// settings: [{ id, label, description, default, kind }] }] }` where `kind` is
/// `{type:"float",min,max,log}`, `{type:"int",min,max}`, `{type:"bool"}` or
/// `{type:"enum",options:[{value,label}]}`.
#[unsafe(no_mangle)]
pub extern "C" fn bender_schema() -> i32 {
    with(|e| {
        let s = schema().to_string();
        write_text(e, &s)
    })
}

fn schema() -> Value {
    let defaults = BendEffect::default();
    let groups: Vec<Value> = setting_groups()
        .iter()
        .map(|g| {
            let settings: Vec<Value> = g
                .settings
                .iter()
                .map(|d| {
                    let kind = match d.kind {
                        SettingKind::Float {
                            min,
                            max,
                            logarithmic,
                        } => json!({ "type": "float", "min": min, "max": max, "log": logarithmic }),
                        SettingKind::Int { min, max } => {
                            json!({ "type": "int", "min": min, "max": max })
                        }
                        SettingKind::Bool => json!({ "type": "bool" }),
                        SettingKind::Enum { options } => json!({
                            "type": "enum",
                            "options": options
                                .iter()
                                .map(|(value, label)| json!({ "value": value, "label": label }))
                                .collect::<Vec<_>>(),
                        }),
                    };
                    let default = match ((d.get)(&defaults), d.kind) {
                        (SettingValue::Float(v), _) => json!(v),
                        (SettingValue::Int(v), _) => json!(v),
                        (SettingValue::Bool(v), _) => json!(v),
                        (SettingValue::Enum(i), SettingKind::Enum { options }) => {
                            json!(options.get(i as usize).map_or("", |o| o.0))
                        }
                        (SettingValue::Enum(i), _) => json!(i),
                    };
                    json!({
                        "id": d.id,
                        "label": d.label,
                        "description": d.description,
                        "default": default,
                        "kind": kind,
                    })
                })
                .collect();
            json!({
                "id": g.id,
                "label": g.label,
                "description": g.description,
                "settings": settings,
            })
        })
        .collect();
    let defaults_json: Value =
        serde_json::from_str(&defaults.to_json()).expect("preset JSON round-trips");
    json!({
        "version": bender::PRESET_VERSION,
        "defaults": defaults_json,
        "groups": groups,
    })
}
