//! Effect settings, introspectable descriptors and JSON presets.
//!
//! [`BendEffect`] is the whole effect: one [`Toggle`]d block per bent circuit, in
//! signal-chain order. [`setting_groups`] describes every field (ids equal the
//! serde JSON paths) for GUIs, command lines and preset clamping.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Current preset format version written by [`BendEffect::to_json`].
pub const PRESET_VERSION: u32 = 1;

/// Errors from loading presets or setting values by id.
#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("invalid preset JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported preset version {0} (this build supports up to {PRESET_VERSION})")]
    UnsupportedVersion(u32),
    #[error("unknown setting '{0}'")]
    UnknownSetting(String),
    #[error("invalid value '{value}' for setting '{id}'")]
    InvalidValue { id: String, value: String },
    #[error("value {value} for setting '{id}' is out of range {min}..={max}")]
    OutOfRange {
        id: String,
        value: String,
        min: String,
        max: String,
    },
}

/// Enumerated setting with stable JSON names and display labels.
pub trait SettingEnum: Copy + 'static {
    /// `(json name, label)` per variant, in index order.
    const OPTIONS: &'static [(&'static str, &'static str)];
    /// Index of this variant in [`Self::OPTIONS`].
    fn index(self) -> u32;
    /// Variant at index `i`, if any.
    fn from_index(i: u32) -> Option<Self>;
}

macro_rules! setting_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = ($json:literal, $label:literal)),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[repr(u32)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            #[default]
            $($(#[$vmeta])* $variant),+
        }

        impl SettingEnum for $name {
            const OPTIONS: &'static [(&'static str, &'static str)] = &[$(($json, $label)),+];
            fn index(self) -> u32 {
                self as u32
            }
            fn from_index(i: u32) -> Option<Self> {
                const ALL: &[$name] = &[$($name::$variant),+];
                ALL.get(i as usize).copied()
            }
        }
    };
}

setting_enum! {
    /// Which signal components a bent data line touches.
    DataTarget {
        /// Luma only.
        Luma = ("luma", "Luma"),
        /// I and Q only.
        Chroma = ("chroma", "Chroma"),
        /// All three components.
        Both = ("both", "Both"),
    }
}

setting_enum! {
    /// How a single data bit is forced.
    StuckMode {
        /// No stuck bit.
        Off = ("off", "Off"),
        /// Shorted to ground.
        Low = ("low", "Stuck low"),
        /// Shorted to the supply.
        High = ("high", "Stuck high"),
        /// Bit inverted.
        Invert = ("invert", "Inverted"),
    }
}

setting_enum! {
    /// Frame-store address axis.
    Axis {
        /// Horizontal (x) address lines.
        Column = ("column", "Column"),
        /// Vertical (y) address lines.
        Row = ("row", "Row"),
    }
}

setting_enum! {
    /// What a bridged address line does to its bit.
    BridgeMode {
        /// Bit forced to 0.
        StuckLow = ("stuck_low", "Stuck low"),
        /// Bit forced to 1.
        StuckHigh = ("stuck_high", "Stuck high"),
        /// Bit XORed with the partner bit.
        Xor = ("xor", "XOR partner"),
        /// Bit ANDed with the partner bit.
        And = ("and", "AND partner"),
        /// Bit ORed with the partner bit.
        Or = ("or", "OR partner"),
    }
}

setting_enum! {
    /// Rewiring applied by the processing amp.
    InvertMode {
        /// Negative luma.
        InvertLuma = ("invert_luma", "Invert luma"),
        /// Complementary hues.
        InvertChroma = ("invert_chroma", "Invert chroma"),
        /// I and Q exchanged.
        SwapIq = ("swap_iq", "Swap I/Q"),
        /// Inversion above a luma threshold.
        Solarize = ("solarize", "Solarize"),
    }
}

setting_enum! {
    /// Signal components an oscillation leaks into.
    OscTarget {
        /// Luma bars.
        Luma = ("luma", "Luma"),
        /// Rainbow chroma bars.
        Chroma = ("chroma", "Chroma"),
        /// Luma and chroma.
        Both = ("both", "Both"),
    }
}

/// A bend block that can be switched in or out of the chain.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Toggle<T> {
    pub enabled: bool,
    #[serde(flatten)]
    pub settings: T,
}

impl<T> Toggle<T> {
    fn off(settings: T) -> Self {
        Self {
            enabled: false,
            settings,
        }
    }
}

/// Starved or shorted sync separator: rows lose horizontal lock and the picture rolls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncBend {
    pub h_skew: f32,
    pub h_jitter: f32,
    pub tear_rate: f32,
    pub v_roll: f32,
}

impl Default for SyncBend {
    fn default() -> Self {
        Self {
            h_skew: 0.0,
            h_jitter: 0.02,
            tear_rate: 0.1,
            v_roll: 0.0,
        }
    }
}

/// Bent ADC pixel clock: samples held, stretched and stalled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClockBend {
    pub sample_hold: i32,
    pub stretch: f32,
    pub glitch_rate: f32,
}

impl Default for ClockBend {
    fn default() -> Self {
        Self {
            sample_hold: 1,
            stretch: 1.0,
            glitch_rate: 0.0,
        }
    }
}

/// Bridged ADC/RAM data bus: swapped and stuck bits in quantised samples.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataBend {
    pub target: DataTarget,
    pub swap_a: i32,
    pub swap_b: i32,
    pub stuck_bit: i32,
    pub stuck_mode: StuckMode,
    pub coverage: f32,
}

impl Default for DataBend {
    fn default() -> Self {
        Self {
            target: DataTarget::Luma,
            swap_a: 7,
            swap_b: 6,
            stuck_bit: 0,
            stuck_mode: StuckMode::Off,
            coverage: 1.0,
        }
    }
}

/// Glitched frame-store write-enable: bands keep the previous frame's contents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryHold {
    pub rate: f32,
    pub band_height: i32,
    pub decay: f32,
}

impl Default for MemoryHold {
    fn default() -> Self {
        Self {
            rate: 0.2,
            band_height: 16,
            decay: 0.0,
        }
    }
}

/// Bridged frame-store read address lines: tiled, mirrored and shuffled reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AddressBridge {
    pub axis: Axis,
    pub bit: i32,
    pub mode: BridgeMode,
    pub partner_axis: Axis,
    pub partner_bit: i32,
    pub coverage: f32,
}

impl Default for AddressBridge {
    fn default() -> Self {
        Self {
            axis: Axis::Column,
            bit: 5,
            mode: BridgeMode::StuckLow,
            partner_axis: Axis::Column,
            partner_bit: 7,
            coverage: 1.0,
        }
    }
}

impl AddressBridge {
    /// Default for the second bridge: row address lines.
    pub fn default_b() -> Self {
        Self {
            axis: Axis::Row,
            partner_axis: Axis::Row,
            ..Self::default()
        }
    }
}

/// Rewired processing amp: inverted, swapped or solarized components.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Inversion {
    pub mode: InvertMode,
    pub threshold: f32,
    pub mix: f32,
}

impl Default for Inversion {
    fn default() -> Self {
        Self {
            mode: InvertMode::Solarize,
            threshold: 0.5,
            mix: 1.0,
        }
    }
}

/// Starved supply rail: brightness-dependent sag, highlight streaks and hum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RailSag {
    pub sag: f32,
    pub streak: f32,
    pub hum: f32,
}

impl Default for RailSag {
    fn default() -> Self {
        Self {
            sag: 0.3,
            streak: 0.3,
            hum: 0.0,
        }
    }
}

/// Self-oscillating op-amp: drifting diagonal interference bars.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Oscillation {
    pub target: OscTarget,
    pub frequency: f32,
    pub slant: f32,
    pub drift: f32,
    pub amplitude: f32,
    pub luma_coupling: f32,
}

impl Default for Oscillation {
    fn default() -> Self {
        Self {
            target: OscTarget::Luma,
            frequency: 0.05,
            slant: 0.3,
            drift: 0.1,
            amplitude: 0.2,
            luma_coupling: 0.5,
        }
    }
}

/// Bent colour-burst / chroma decoder: hue errors, saturation and colour killing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BurstBend {
    pub phase: f32,
    pub wobble: f32,
    pub saturation: f32,
    pub killer_rate: f32,
}

impl Default for BurstBend {
    fn default() -> Self {
        Self {
            phase: 0.0,
            wobble: 0.0,
            saturation: 1.0,
            killer_rate: 0.0,
        }
    }
}

/// Narrow, misaligned chroma path: colour smears right and slips out of register with luma.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromaBleed {
    pub smear: f32,
    pub shift: f32,
}

impl Default for ChromaBleed {
    fn default() -> Self {
        Self {
            smear: 0.02,
            shift: 0.01,
        }
    }
}

/// The complete circuit-bending effect; blocks run in field order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BendEffect {
    pub version: u32,
    pub seed: i32,
    pub sync: Toggle<SyncBend>,
    pub clock: Toggle<ClockBend>,
    pub data: Toggle<DataBend>,
    pub hold: Toggle<MemoryHold>,
    pub address_a: Toggle<AddressBridge>,
    pub address_b: Toggle<AddressBridge>,
    pub invert: Toggle<Inversion>,
    pub rail: Toggle<RailSag>,
    pub osc: Toggle<Oscillation>,
    pub burst: Toggle<BurstBend>,
    pub bleed: Toggle<ChromaBleed>,
}

impl Default for BendEffect {
    /// All blocks disabled: the identity effect.
    fn default() -> Self {
        Self {
            version: PRESET_VERSION,
            seed: 0,
            sync: Toggle::off(SyncBend::default()),
            clock: Toggle::off(ClockBend::default()),
            data: Toggle::off(DataBend::default()),
            hold: Toggle::off(MemoryHold::default()),
            address_a: Toggle::off(AddressBridge::default()),
            address_b: Toggle::off(AddressBridge::default_b()),
            invert: Toggle::off(Inversion::default()),
            rail: Toggle::off(RailSag::default()),
            osc: Toggle::off(Oscillation::default()),
            burst: Toggle::off(BurstBend::default()),
            bleed: Toggle::off(ChromaBleed::default()),
        }
    }
}

/// Type and range of a setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingKind {
    Float {
        min: f32,
        max: f32,
        logarithmic: bool,
    },
    Int {
        min: i32,
        max: i32,
    },
    Bool,
    Enum {
        options: &'static [(&'static str, &'static str)],
    },
}

/// A setting value; enums are carried as their [`SettingEnum::index`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Enum(u32),
}

/// One introspectable setting. `id` is its JSON path, e.g. `"sync.h_skew"`.
pub struct SettingDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub kind: SettingKind,
    pub get: fn(&BendEffect) -> SettingValue,
    /// Ignores values of the wrong variant.
    pub set: fn(&mut BendEffect, SettingValue),
}

/// One bend block; its first setting is always `<id>.enabled`.
pub struct SettingGroup {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub settings: Vec<SettingDescriptor>,
}

macro_rules! enabled {
    ($g:ident, $label:literal) => {
        SettingDescriptor {
            id: concat!(stringify!($g), ".enabled"),
            label: "Enabled",
            description: concat!("Switch the ", $label, " bend into the chain."),
            kind: SettingKind::Bool,
            get: |e| SettingValue::Bool(e.$g.enabled),
            set: |e, v| {
                if let SettingValue::Bool(v) = v {
                    e.$g.enabled = v;
                }
            },
        }
    };
}

macro_rules! float {
    ($g:ident . $f:ident, $label:literal, $desc:literal, $min:expr, $max:expr $(, $log:ident)?) => {
        SettingDescriptor {
            id: concat!(stringify!($g), ".", stringify!($f)),
            label: $label,
            description: $desc,
            kind: SettingKind::Float {
                min: $min,
                max: $max,
                logarithmic: float!(@log $($log)?),
            },
            get: |e| SettingValue::Float(e.$g.settings.$f),
            set: |e, v| {
                if let SettingValue::Float(v) = v {
                    e.$g.settings.$f = v;
                }
            },
        }
    };
    (@log) => { false };
    (@log log) => { true };
}

macro_rules! int {
    ($g:ident . $f:ident, $label:literal, $desc:literal, $min:expr, $max:expr) => {
        SettingDescriptor {
            id: concat!(stringify!($g), ".", stringify!($f)),
            label: $label,
            description: $desc,
            kind: SettingKind::Int {
                min: $min,
                max: $max,
            },
            get: |e| SettingValue::Int(e.$g.settings.$f),
            set: |e, v| {
                if let SettingValue::Int(v) = v {
                    e.$g.settings.$f = v;
                }
            },
        }
    };
}

macro_rules! enumeration {
    ($g:ident . $f:ident, $ty:ty, $label:literal, $desc:literal) => {
        SettingDescriptor {
            id: concat!(stringify!($g), ".", stringify!($f)),
            label: $label,
            description: $desc,
            kind: SettingKind::Enum {
                options: <$ty as SettingEnum>::OPTIONS,
            },
            get: |e| SettingValue::Enum(e.$g.settings.$f.index()),
            set: |e, v| {
                if let SettingValue::Enum(i) = v
                    && let Some(v) = <$ty as SettingEnum>::from_index(i)
                {
                    e.$g.settings.$f = v;
                }
            },
        }
    };
}

macro_rules! address_group {
    ($g:ident, $label:literal, $desc:literal) => {
        SettingGroup {
            id: stringify!($g),
            label: $label,
            description: $desc,
            settings: vec![
                enabled!($g, $label),
                enumeration!(
                    $g.axis,
                    Axis,
                    "Axis",
                    "Address lines (column = x, row = y) whose bit is bridged."
                ),
                int!(
                    $g.bit,
                    "Bit",
                    "Address bit that is altered; bit k affects 2^k-pixel blocks.",
                    0,
                    10
                ),
                enumeration!(
                    $g.mode,
                    BridgeMode,
                    "Mode",
                    "What the bridge does: force the bit, or combine it with the partner bit."
                ),
                enumeration!(
                    $g.partner_axis,
                    Axis,
                    "Partner axis",
                    "Axis of the partner address line used by XOR/AND/OR."
                ),
                int!(
                    $g.partner_bit,
                    "Partner bit",
                    "Partner address bit used by XOR/AND/OR.",
                    0,
                    10
                ),
                float!(
                    $g.coverage,
                    "Coverage",
                    "Probability that each 8-row band makes contact.",
                    0.0,
                    1.0
                ),
            ],
        }
    };
}

/// All setting groups in signal-chain order.
pub fn setting_groups() -> &'static [SettingGroup] {
    static GROUPS: LazyLock<Vec<SettingGroup>> = LazyLock::new(build_groups);
    &GROUPS
}

fn build_groups() -> Vec<SettingGroup> {
    vec![
        SettingGroup {
            id: "sync",
            label: "Sync separator",
            description: "A starved or shorted sync separator: lines lose horizontal lock, tear and the picture rolls.",
            settings: vec![
                enabled!(sync, "Sync separator"),
                float!(
                    sync.h_skew,
                    "Skew",
                    "Horizontal shift per row as a fraction of width; slants the picture diagonally.",
                    -1.0,
                    1.0
                ),
                float!(
                    sync.h_jitter,
                    "Jitter",
                    "Random per-row horizontal shift, as a fraction of width.",
                    0.0,
                    0.2
                ),
                float!(
                    sync.tear_rate,
                    "Tear rate",
                    "Per-frame probability of a band of torn, brightened lines.",
                    0.0,
                    1.0
                ),
                float!(
                    sync.v_roll,
                    "Vertical roll",
                    "Vertical roll speed with a visible blanking bar; 1 = 5% of the height per frame.",
                    -1.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "clock",
            label: "Pixel clock",
            description: "A bent ADC pixel clock: samples are held, the sample rate is stretched and the clock stalls.",
            settings: vec![
                enabled!(clock, "Pixel clock"),
                int!(
                    clock.sample_hold,
                    "Sample hold",
                    "Each sample is held across this many pixels.",
                    1,
                    64
                ),
                float!(
                    clock.stretch,
                    "Stretch",
                    "Sample-rate multiplier; above 1 repeats the image horizontally.",
                    0.25,
                    4.0,
                    log
                ),
                float!(
                    clock.glitch_rate,
                    "Glitch rate",
                    "Per-row probability the clock stalls and smears one sample.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "data",
            label: "Data bus",
            description: "Bridged ADC/RAM data lines: bits of each 8-bit sample are swapped or stuck.",
            settings: vec![
                enabled!(data, "Data bus"),
                enumeration!(
                    data.target,
                    DataTarget,
                    "Target",
                    "Which components pass over the bent bus."
                ),
                int!(
                    data.swap_a,
                    "Swap bit A",
                    "First of two data bits that are swapped (equal bits = no swap).",
                    0,
                    7
                ),
                int!(
                    data.swap_b,
                    "Swap bit B",
                    "Second of two data bits that are swapped.",
                    0,
                    7
                ),
                int!(
                    data.stuck_bit,
                    "Stuck bit",
                    "Data bit forced by the stuck mode.",
                    0,
                    7
                ),
                enumeration!(
                    data.stuck_mode,
                    StuckMode,
                    "Stuck mode",
                    "How the stuck bit is forced."
                ),
                float!(
                    data.coverage,
                    "Coverage",
                    "Probability that each 8-row band makes contact.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "hold",
            label: "Memory hold",
            description: "A glitched frame-store write-enable: bands of rows keep the previous frame.",
            settings: vec![
                enabled!(hold, "Memory hold"),
                float!(
                    hold.rate,
                    "Rate",
                    "Per-band probability the band is not rewritten this frame.",
                    0.0,
                    1.0
                ),
                int!(
                    hold.band_height,
                    "Band height",
                    "Rows per write band.",
                    1,
                    120
                ),
                float!(
                    hold.decay,
                    "Decay",
                    "How quickly held rows fade towards grey.",
                    0.0,
                    1.0
                ),
            ],
        },
        address_group!(
            address_a,
            "Address bridge A",
            "A bridged frame-store read address line: tiles, mirrors and shuffles blocks."
        ),
        address_group!(
            address_b,
            "Address bridge B",
            "A second bridged read address line, by default on the row address."
        ),
        SettingGroup {
            id: "invert",
            label: "Inversion",
            description: "A rewired processing amp: inverted luma or chroma, swapped I/Q, or solarization.",
            settings: vec![
                enabled!(invert, "Inversion"),
                enumeration!(
                    invert.mode,
                    InvertMode,
                    "Mode",
                    "Which rewiring is applied."
                ),
                float!(
                    invert.threshold,
                    "Threshold",
                    "Luma level above which solarization inverts.",
                    0.0,
                    1.0
                ),
                float!(
                    invert.mix,
                    "Mix",
                    "Blend between the original and the inverted signal.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "rail",
            label: "Rail sag",
            description: "A starved supply rail: bright lines dim and shrink, highlights streak, mains hum rolls.",
            settings: vec![
                enabled!(rail, "Rail sag"),
                float!(
                    rail.sag,
                    "Sag",
                    "Brightness-dependent loss of gain and picture width.",
                    0.0,
                    1.0
                ),
                float!(
                    rail.streak,
                    "Streak",
                    "Overload smear trailing to the right of highlights.",
                    0.0,
                    1.0
                ),
                float!(
                    rail.hum,
                    "Hum",
                    "Depth of the rolling mains hum bar.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "osc",
            label: "Oscillation",
            description: "A self-oscillating op-amp: drifting diagonal interference bars.",
            settings: vec![
                enabled!(osc, "Oscillation"),
                enumeration!(
                    osc.target,
                    OscTarget,
                    "Target",
                    "Which components the oscillation leaks into."
                ),
                float!(
                    osc.frequency,
                    "Frequency",
                    "Oscillation frequency in cycles per pixel.",
                    0.001,
                    0.5,
                    log
                ),
                float!(
                    osc.slant,
                    "Slant",
                    "Phase advance per row in cycles; tilts the bars.",
                    -2.0,
                    2.0
                ),
                float!(
                    osc.drift,
                    "Drift",
                    "Phase advance per frame in cycles; makes the bars crawl.",
                    -1.0,
                    1.0
                ),
                float!(
                    osc.amplitude,
                    "Amplitude",
                    "Strength of the interference.",
                    0.0,
                    1.0
                ),
                float!(
                    osc.luma_coupling,
                    "Luma coupling",
                    "How much the amplitude follows picture brightness.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "burst",
            label: "Colour burst",
            description: "A bent colour-burst / chroma decoder: hue shifts, phase wobble, saturation and colour killing.",
            settings: vec![
                enabled!(burst, "Colour burst"),
                float!(
                    burst.phase,
                    "Phase",
                    "Hue rotation in degrees.",
                    -180.0,
                    180.0
                ),
                float!(
                    burst.wobble,
                    "Wobble",
                    "Per-row phase error amplitude in degrees.",
                    0.0,
                    180.0
                ),
                float!(burst.saturation, "Saturation", "Chroma gain.", 0.0, 4.0),
                float!(
                    burst.killer_rate,
                    "Killer rate",
                    "Per-8-row-band probability the colour killer blanks chroma.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "bleed",
            label: "Chroma bleed",
            description: "A narrow, misaligned chroma path: colour smears to the right of edges and slips out of register with luma.",
            settings: vec![
                enabled!(bleed, "Chroma bleed"),
                float!(
                    bleed.smear,
                    "Smear",
                    "Chroma low-pass time constant as a fraction of width; colour trails to the right of edges.",
                    0.0,
                    0.25
                ),
                float!(
                    bleed.shift,
                    "Shift",
                    "Chroma delay relative to luma as a fraction of width; positive moves colour right.",
                    -0.1,
                    0.1
                ),
            ],
        },
    ]
}

/// Looks up a descriptor by id (e.g. `"sync.h_skew"`).
pub fn find_setting(id: &str) -> Option<&'static SettingDescriptor> {
    setting_groups()
        .iter()
        .flat_map(|g| g.settings.iter())
        .find(|d| d.id == id)
}

impl BendEffect {
    /// Serializes to pretty-printed preset JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("BendEffect serializes infallibly")
    }

    /// Parses preset JSON. Missing fields take defaults, unknown fields are
    /// ignored, and out-of-range values are clamped.
    pub fn from_json(s: &str) -> Result<Self, PresetError> {
        let value: serde_json::Value = serde_json::from_str(s)?;
        if let Some(v) = value.get("version").and_then(serde_json::Value::as_u64)
            && v > u64::from(PRESET_VERSION)
        {
            return Err(PresetError::UnsupportedVersion(
                u32::try_from(v).unwrap_or(u32::MAX),
            ));
        }
        let mut effect: Self = serde_json::from_value(value)?;
        effect.version = PRESET_VERSION;
        effect.clamp_to_ranges();
        Ok(effect)
    }

    /// Clamps every setting into its descriptor's range.
    pub fn clamp_to_ranges(&mut self) {
        for d in setting_groups().iter().flat_map(|g| g.settings.iter()) {
            let clamped = match (d.kind, (d.get)(self)) {
                (SettingKind::Float { min, max, .. }, SettingValue::Float(v)) => {
                    SettingValue::Float(if v.is_nan() { min } else { v.clamp(min, max) })
                }
                (SettingKind::Int { min, max }, SettingValue::Int(v)) => {
                    SettingValue::Int(v.clamp(min, max))
                }
                (SettingKind::Enum { options }, SettingValue::Enum(i)) => {
                    SettingValue::Enum(if (i as usize) < options.len() { i } else { 0 })
                }
                (_, v) => v,
            };
            (d.set)(self, clamped);
        }
    }

    /// Sets a setting from text: floats, integers, `true`/`false`, or an enum's
    /// JSON name. `"seed"` sets [`BendEffect::seed`].
    pub fn set_by_id(&mut self, id: &str, raw: &str) -> Result<(), PresetError> {
        let raw = raw.trim();
        let invalid = || PresetError::InvalidValue {
            id: id.to_owned(),
            value: raw.to_owned(),
        };
        if id == "seed" {
            self.seed = raw.parse().map_err(|_| invalid())?;
            return Ok(());
        }
        let d = find_setting(id).ok_or_else(|| PresetError::UnknownSetting(id.to_owned()))?;
        let out_of_range = |min: String, max: String| PresetError::OutOfRange {
            id: id.to_owned(),
            value: raw.to_owned(),
            min,
            max,
        };
        let value = match d.kind {
            SettingKind::Float { min, max, .. } => {
                let v: f32 = raw.parse().map_err(|_| invalid())?;
                if !(min..=max).contains(&v) {
                    return Err(out_of_range(min.to_string(), max.to_string()));
                }
                SettingValue::Float(v)
            }
            SettingKind::Int { min, max } => {
                let v: i32 = raw.parse().map_err(|_| invalid())?;
                if !(min..=max).contains(&v) {
                    return Err(out_of_range(min.to_string(), max.to_string()));
                }
                SettingValue::Int(v)
            }
            SettingKind::Bool => SettingValue::Bool(raw.parse().map_err(|_| invalid())?),
            SettingKind::Enum { options } => {
                let i = options
                    .iter()
                    .position(|(name, _)| *name == raw)
                    .ok_or_else(invalid)?;
                SettingValue::Enum(i as u32)
            }
        };
        (d.set)(self, value);
        Ok(())
    }
}
