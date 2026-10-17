//! Effect settings, introspectable descriptors and JSON presets.
//!
//! [`BendEffect`] is the whole effect: one [`Toggle`]d block per bent circuit,
//! run in its [`ChainOrder`]. [`setting_groups`] describes every field (ids
//! equal the serde JSON paths) for GUIs, command lines and preset clamping.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Current preset format version written by [`BendEffect::to_json`].
pub const PRESET_VERSION: u32 = 2;

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

setting_enum! {
    /// How the two bridged data bits interact.
    PairMode {
        /// The bits trade places.
        Swap = ("swap", "Swap"),
        /// Bit A overwrites bit B.
        Copy = ("copy", "A drives B"),
        /// Both bits read their AND.
        And = ("and", "Wired AND"),
        /// Both bits read their OR.
        Or = ("or", "Wired OR"),
    }
}

setting_enum! {
    /// Which part of the frame store misses its write.
    HoldMode {
        /// Random bands of rows.
        Bands = ("bands", "Bands"),
        /// Whole even or odd fields.
        Fields = ("fields", "Fields"),
        /// The whole store, written only every few frames.
        Strobe = ("strobe", "Strobe"),
    }
}

setting_enum! {
    /// Colour-filter phase error between the sensor and the demosaic.
    BayerPhase {
        /// Filter and demosaic agree.
        Off = ("off", "Off"),
        /// Demosaic shifted one pixel horizontally.
        Horizontal = ("horizontal", "Shifted 1 px"),
        /// Demosaic shifted one row.
        Vertical = ("vertical", "Shifted 1 row"),
        /// Demosaic shifted both ways.
        Diagonal = ("diagonal", "Shifted both"),
    }
}

setting_enum! {
    /// Edge-enhancement filter of the retina sensor.
    RetinaEdge {
        /// Four-neighbour Laplacian.
        TwoD = ("two_d", "2-D"),
        /// Left/right neighbours only.
        Horizontal = ("horizontal", "Horizontal"),
        /// No enhancement.
        Off = ("off", "Off"),
    }
}

setting_enum! {
    /// Display palette of the retina camera.
    RetinaPalette {
        /// The original handheld's four greens.
        Dmg = ("dmg", "DMG green"),
        /// Neutral greys.
        Grey = ("grey", "Grey"),
    }
}

setting_enum! {
    /// Luma/chroma separation in the composite decoder.
    Decoder {
        /// Subcarrier notch filter.
        Notch = ("notch", "Notch"),
        /// One-line comb filter.
        Comb = ("comb", "1H comb"),
    }
}

setting_enum! {
    /// How the colourizer paints luma.
    ColorizeMode {
        /// Flat colour per luma band.
        Bands = ("bands", "Bands"),
        /// Continuous hue sweep.
        Gradient = ("gradient", "Gradient"),
    }
}

/// One bent circuit. Declaration order is the default signal chain:
/// camera → tape → TBC → ADC → codec → frame store → proc amp → encoder → monitor → loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageId {
    Sensor,
    Retina,
    Isp,
    Tape,
    Sync,
    Clock,
    Data,
    Dct,
    Mosh,
    Hold,
    Counter,
    AddressA,
    AddressB,
    Colorize,
    Invert,
    Rail,
    Osc,
    Burst,
    Bleed,
    Composite,
    Scan,
    Feedback,
}

impl StageId {
    /// Every stage in default chain order; `id as usize` indexes this and [`setting_groups`].
    pub const ALL: [StageId; 22] = [
        StageId::Sensor,
        StageId::Retina,
        StageId::Isp,
        StageId::Tape,
        StageId::Sync,
        StageId::Clock,
        StageId::Data,
        StageId::Dct,
        StageId::Mosh,
        StageId::Hold,
        StageId::Counter,
        StageId::AddressA,
        StageId::AddressB,
        StageId::Colorize,
        StageId::Invert,
        StageId::Rail,
        StageId::Osc,
        StageId::Burst,
        StageId::Bleed,
        StageId::Composite,
        StageId::Scan,
        StageId::Feedback,
    ];

    /// JSON name, equal to the setting group id.
    pub const fn name(self) -> &'static str {
        match self {
            StageId::Sensor => "sensor",
            StageId::Retina => "retina",
            StageId::Isp => "isp",
            StageId::Tape => "tape",
            StageId::Sync => "sync",
            StageId::Clock => "clock",
            StageId::Data => "data",
            StageId::Dct => "dct",
            StageId::Mosh => "mosh",
            StageId::Hold => "hold",
            StageId::Counter => "counter",
            StageId::AddressA => "address_a",
            StageId::AddressB => "address_b",
            StageId::Colorize => "colorize",
            StageId::Invert => "invert",
            StageId::Rail => "rail",
            StageId::Osc => "osc",
            StageId::Burst => "burst",
            StageId::Bleed => "bleed",
            StageId::Composite => "composite",
            StageId::Scan => "scan",
            StageId::Feedback => "feedback",
        }
    }

    /// Stage with JSON name `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// Stable random-stream id; never renumber, presets depend on it.
    pub(crate) const fn rng_id(self) -> u64 {
        match self {
            StageId::Sync => 1,
            StageId::Clock => 2,
            StageId::Data => 3,
            StageId::Hold => 4,
            StageId::AddressA => 5,
            StageId::AddressB => 6,
            StageId::Invert => 7,
            StageId::Rail => 8,
            StageId::Osc => 9,
            StageId::Burst => 10,
            StageId::Bleed => 11,
            StageId::Sensor => 12,
            StageId::Retina => 13,
            StageId::Isp => 14,
            StageId::Tape => 15,
            StageId::Dct => 16,
            StageId::Mosh => 17,
            StageId::Counter => 18,
            StageId::Colorize => 19,
            StageId::Composite => 20,
            StageId::Scan => 21,
            StageId::Feedback => 22,
        }
    }
}

/// Order in which stages run; always a permutation of [`StageId::ALL`].
///
/// Serialized as a JSON array of stage names. Unknown names are dropped and
/// missing stages follow in default order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainOrder(Vec<StageId>);

impl Default for ChainOrder {
    fn default() -> Self {
        Self(StageId::ALL.to_vec())
    }
}

impl ChainOrder {
    /// Stages in run order.
    pub fn stages(&self) -> &[StageId] {
        &self.0
    }

    /// Moves `id` by `delta` places, clamped to the ends of the chain.
    pub fn move_stage(&mut self, id: StageId, delta: isize) {
        let Some(old) = self.0.iter().position(|&s| s == id) else {
            return;
        };
        let new = (old as isize + delta).clamp(0, self.0.len() as isize - 1) as usize;
        let s = self.0.remove(old);
        self.0.insert(new, s);
    }

    /// Keeps the first occurrence of each id, then appends missing ids in default order.
    fn normalized(ids: impl IntoIterator<Item = StageId>) -> Self {
        let mut v = Vec::with_capacity(StageId::ALL.len());
        for id in ids.into_iter().chain(StageId::ALL) {
            if !v.contains(&id) {
                v.push(id);
            }
        }
        Self(v)
    }

    /// Parses a comma-separated list of stage names; listed stages run first,
    /// the rest follow in default order. Unknown or repeated names are rejected
    /// with the offending name.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut ids = Vec::new();
        for name in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let id = StageId::from_name(name).ok_or_else(|| name.to_owned())?;
            if ids.contains(&id) {
                return Err(name.to_owned());
            }
            ids.push(id);
        }
        Ok(Self::normalized(ids))
    }
}

impl std::fmt::Display for ChainOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (k, id) in self.0.iter().enumerate() {
            if k > 0 {
                f.write_str(",")?;
            }
            f.write_str(id.name())?;
        }
        Ok(())
    }
}

impl Serialize for ChainOrder {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(&self.0)
    }
}

impl<'de> Deserialize<'de> for ChainOrder {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let names = Vec::<String>::deserialize(deserializer)?;
        Ok(Self::normalized(
            names.iter().filter_map(|n| StageId::from_name(n)),
        ))
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
    pub jitter_smooth: f32,
    pub tear_rate: f32,
    pub v_roll: f32,
    pub blanking: bool,
    pub clamp_error: f32,
}

impl Default for SyncBend {
    fn default() -> Self {
        Self {
            h_skew: 0.0,
            h_jitter: 0.02,
            jitter_smooth: 0.8,
            tear_rate: 0.1,
            v_roll: 0.0,
            blanking: true,
            clamp_error: 0.0,
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
    pub jitter: f32,
    pub grid_drift: f32,
}

impl Default for ClockBend {
    fn default() -> Self {
        Self {
            sample_hold: 1,
            stretch: 1.0,
            glitch_rate: 0.0,
            jitter: 0.0,
            grid_drift: 0.0,
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
    pub pair_mode: PairMode,
    pub stuck_bit: i32,
    pub stuck_mode: StuckMode,
    pub lag_bit: i32,
    pub lag: i32,
    pub coverage: f32,
}

impl Default for DataBend {
    fn default() -> Self {
        Self {
            target: DataTarget::Luma,
            swap_a: 7,
            swap_b: 6,
            pair_mode: PairMode::Swap,
            stuck_bit: 0,
            stuck_mode: StuckMode::Off,
            lag_bit: 8,
            lag: 2,
            coverage: 1.0,
        }
    }
}

/// Glitched frame-store write-enable: bands keep the previous frame's contents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryHold {
    pub mode: HoldMode,
    pub rate: f32,
    pub band_height: i32,
    pub strobe_period: i32,
    pub decay: f32,
}

impl Default for MemoryHold {
    fn default() -> Self {
        Self {
            mode: HoldMode::Bands,
            rate: 0.2,
            band_height: 16,
            strobe_period: 4,
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
    pub hum_bend: f32,
    pub bars: f32,
    pub bar_period: i32,
}

impl Default for RailSag {
    fn default() -> Self {
        Self {
            sag: 0.3,
            streak: 0.3,
            hum: 0.0,
            hum_bend: 0.3,
            bars: 0.0,
            bar_period: 8,
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
    pub ringing: f32,
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
            ringing: 0.0,
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
    pub hanover: f32,
    pub alternate_loss: f32,
}

impl Default for BurstBend {
    fn default() -> Self {
        Self {
            phase: 0.0,
            wobble: 0.0,
            saturation: 1.0,
            killer_rate: 0.0,
            hanover: 0.0,
            alternate_loss: 0.0,
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

/// Bent CCD/CMOS imager: rolling-shutter skew and slip, smear, blooming, CFA phase and fixed-pattern noise.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SensorFault {
    pub skew: f32,
    pub slip: f32,
    pub flicker: f32,
    pub smear: f32,
    pub bloom: f32,
    pub bayer: BayerPhase,
    pub fpn: f32,
}

impl Default for SensorFault {
    fn default() -> Self {
        Self {
            skew: 0.5,
            slip: 0.0,
            flicker: 0.0,
            smear: 0.3,
            bloom: 0.2,
            bayer: BayerPhase::Off,
            fpn: 0.1,
        }
    }
}

/// Game Boy Camera retina: tiny grid, edge enhancement, dithered 2-bit output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetinaCamera {
    pub resolution: i32,
    pub edge_mode: RetinaEdge,
    pub edge: i32,
    pub levels: i32,
    pub dither: f32,
    pub read_slow: f32,
    pub invert: bool,
    pub palette: RetinaPalette,
}

impl Default for RetinaCamera {
    fn default() -> Self {
        Self {
            resolution: 112,
            edge_mode: RetinaEdge::TwoD,
            edge: 2,
            levels: 4,
            dither: 1.0,
            read_slow: 0.0,
            invert: false,
            palette: RetinaPalette::Dmg,
        }
    }
}

/// Camera DSP with bent control loops: exposure pumping, white-balance hunting, aperture halos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraIsp {
    pub hunt: f32,
    pub white_balance: f32,
    pub sharpen: f32,
    pub sharpen_radius: i32,
    pub noise: f32,
}

impl Default for CameraIsp {
    fn default() -> Self {
        Self {
            hunt: 0.5,
            white_balance: 0.3,
            sharpen: 0.5,
            sharpen_radius: 1,
            noise: 0.1,
        }
    }
}

/// Worn VHS playback: head switching, tracking noise, dropouts, soft luma and colour-under chroma.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TapePlayback {
    pub head_switch: f32,
    pub flagging: f32,
    pub tracking: f32,
    pub dropout: f32,
    pub doc: bool,
    pub comet: f32,
    pub luma_bw: f32,
    pub chroma_noise: f32,
    pub jitter: f32,
}

impl Default for TapePlayback {
    fn default() -> Self {
        Self {
            head_switch: 0.5,
            flagging: 0.0,
            tracking: 0.3,
            dropout: 0.1,
            doc: false,
            comet: 0.0,
            luma_bw: 0.5,
            chroma_noise: 0.4,
            jitter: 0.2,
        }
    }
}

/// NTSC composite encode, cable and cheap decoder: dot crawl, rainbows, ghosts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompositeLink {
    pub decoder: Decoder,
    pub trap: f32,
    pub detune: f32,
    pub dot_crawl: bool,
    pub peaking: f32,
    pub ghost: f32,
    pub noise: f32,
}

impl Default for CompositeLink {
    fn default() -> Self {
        Self {
            decoder: Decoder::Notch,
            trap: 1.0,
            detune: 0.0,
            dot_crawl: true,
            peaking: 0.0,
            ghost: 0.0,
            noise: 0.0,
        }
    }
}

/// 8×8 DCT block codec with a bent quantizer ROM and corrupted bitstream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DctCodec {
    pub quality: i32,
    pub table_shift: i32,
    pub scramble: f32,
    pub dc_drift: f32,
    pub block_shuffle: f32,
}

impl Default for DctCodec {
    fn default() -> Self {
        Self {
            quality: 30,
            table_shift: 0,
            scramble: 0.0,
            dc_drift: 0.0,
            block_shuffle: 0.0,
        }
    }
}

/// Decoder that lost its keyframes: live motion drags the stale picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Datamosh {
    pub keyframe_rate: f32,
    pub search: i32,
    pub vector_gain: f32,
    pub residual: f32,
}

impl Default for Datamosh {
    fn default() -> Self {
        Self {
            keyframe_rate: 0.02,
            search: 8,
            vector_gain: 1.0,
            residual: 0.25,
        }
    }
}

/// Bent frame-store address counters: wrong pitch, early resets, creeping start address.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AddressCounter {
    pub pitch: i32,
    pub line_length: f32,
    pub frame_length: f32,
    pub scroll: f32,
}

impl Default for AddressCounter {
    fn default() -> Self {
        Self {
            pitch: 0,
            line_length: 1.0,
            frame_length: 1.0,
            scroll: 0.0,
        }
    }
}

/// Video-synth colourizer and luma keyer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Colorizer {
    pub mode: ColorizeMode,
    pub bands: i32,
    pub fold: i32,
    pub hue: f32,
    pub spread: f32,
    pub saturation: f32,
    pub key: f32,
    pub mix: f32,
}

impl Default for Colorizer {
    fn default() -> Self {
        Self {
            mode: ColorizeMode::Bands,
            bands: 6,
            fold: 0,
            hue: 0.0,
            spread: 1.0,
            saturation: 0.6,
            key: 0.0,
            mix: 1.0,
        }
    }
}

/// Rutt/Etra scan processor and wobbulator: deflection driven by the picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanProcessor {
    pub displace: f32,
    pub line_spacing: i32,
    pub thickness: i32,
    pub ac_coupled: bool,
    pub wobble: f32,
    pub wobble_freq: f32,
    pub mix: f32,
}

impl Default for ScanProcessor {
    fn default() -> Self {
        Self {
            displace: 0.5,
            line_spacing: 4,
            thickness: 1,
            ac_coupled: false,
            wobble: 0.0,
            wobble_freq: 0.002,
            mix: 1.0,
        }
    }
}

/// Camera pointed at its own monitor: zoomed, rotated, hue-shifted video feedback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeedbackLoop {
    pub amount: f32,
    pub zoom: f32,
    pub rotate: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub hue_shift: f32,
    pub gain: f32,
    pub blur: f32,
    pub key: f32,
}

impl Default for FeedbackLoop {
    fn default() -> Self {
        Self {
            amount: 0.6,
            zoom: 1.03,
            rotate: 2.0,
            offset_x: 0.0,
            offset_y: 0.0,
            hue_shift: 10.0,
            gain: 1.0,
            blur: 0.3,
            key: 0.0,
        }
    }
}

/// The complete circuit-bending effect; enabled blocks run in [`BendEffect::order`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BendEffect {
    pub version: u32,
    pub seed: i32,
    pub order: ChainOrder,
    pub sensor: Toggle<SensorFault>,
    pub retina: Toggle<RetinaCamera>,
    pub isp: Toggle<CameraIsp>,
    pub tape: Toggle<TapePlayback>,
    pub sync: Toggle<SyncBend>,
    pub clock: Toggle<ClockBend>,
    pub data: Toggle<DataBend>,
    pub dct: Toggle<DctCodec>,
    pub mosh: Toggle<Datamosh>,
    pub hold: Toggle<MemoryHold>,
    pub counter: Toggle<AddressCounter>,
    pub address_a: Toggle<AddressBridge>,
    pub address_b: Toggle<AddressBridge>,
    pub colorize: Toggle<Colorizer>,
    pub invert: Toggle<Inversion>,
    pub rail: Toggle<RailSag>,
    pub osc: Toggle<Oscillation>,
    pub burst: Toggle<BurstBend>,
    pub bleed: Toggle<ChromaBleed>,
    pub composite: Toggle<CompositeLink>,
    pub scan: Toggle<ScanProcessor>,
    pub feedback: Toggle<FeedbackLoop>,
}

impl Default for BendEffect {
    /// All blocks disabled: the identity effect.
    fn default() -> Self {
        Self {
            version: PRESET_VERSION,
            seed: 0,
            order: ChainOrder::default(),
            sensor: Toggle::off(SensorFault::default()),
            retina: Toggle::off(RetinaCamera::default()),
            isp: Toggle::off(CameraIsp::default()),
            tape: Toggle::off(TapePlayback::default()),
            sync: Toggle::off(SyncBend::default()),
            clock: Toggle::off(ClockBend::default()),
            data: Toggle::off(DataBend::default()),
            dct: Toggle::off(DctCodec::default()),
            mosh: Toggle::off(Datamosh::default()),
            hold: Toggle::off(MemoryHold::default()),
            counter: Toggle::off(AddressCounter::default()),
            address_a: Toggle::off(AddressBridge::default()),
            address_b: Toggle::off(AddressBridge::default_b()),
            colorize: Toggle::off(Colorizer::default()),
            invert: Toggle::off(Inversion::default()),
            rail: Toggle::off(RailSag::default()),
            osc: Toggle::off(Oscillation::default()),
            burst: Toggle::off(BurstBend::default()),
            bleed: Toggle::off(ChromaBleed::default()),
            composite: Toggle::off(CompositeLink::default()),
            scan: Toggle::off(ScanProcessor::default()),
            feedback: Toggle::off(FeedbackLoop::default()),
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

macro_rules! boolean {
    ($g:ident . $f:ident, $label:literal, $desc:literal) => {
        SettingDescriptor {
            id: concat!(stringify!($g), ".", stringify!($f)),
            label: $label,
            description: $desc,
            kind: SettingKind::Bool,
            get: |e| SettingValue::Bool(e.$g.settings.$f),
            set: |e, v| {
                if let SettingValue::Bool(v) = v {
                    e.$g.settings.$f = v;
                }
            },
        }
    };
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

/// All setting groups in default chain order ([`StageId::ALL`]).
pub fn setting_groups() -> &'static [SettingGroup] {
    static GROUPS: LazyLock<Vec<SettingGroup>> = LazyLock::new(build_groups);
    &GROUPS
}

/// Setting group of stage `id`.
pub fn group(id: StageId) -> &'static SettingGroup {
    &setting_groups()[id as usize]
}

fn build_groups() -> Vec<SettingGroup> {
    vec![
        SettingGroup {
            id: "sensor",
            label: "Image sensor",
            description: "A bent CCD/CMOS imager: rolling-shutter skew and slip, smear and blooming from highlights, a mis-phased colour filter, and fixed-pattern noise.",
            settings: vec![
                enabled!(sensor, "Image sensor"),
                float!(
                    sensor.skew,
                    "Skew",
                    "Rolling-shutter readout time: upper rows blend toward the previous frame.",
                    0.0,
                    1.0
                ),
                float!(
                    sensor.slip,
                    "Slip",
                    "Readout clock slip: rows below a moving split come from the previous frame; 1 = 5% of height per frame.",
                    -1.0,
                    1.0
                ),
                float!(
                    sensor.flicker,
                    "Flicker",
                    "Rolling exposure under mains lighting: crawling horizontal bands.",
                    0.0,
                    1.0
                ),
                float!(
                    sensor.smear,
                    "Smear",
                    "CCD vertical smear: full-height stripes through highlights.",
                    0.0,
                    1.0
                ),
                float!(
                    sensor.bloom,
                    "Bloom",
                    "Charge overflow spreading vertically from highlights.",
                    0.0,
                    1.0
                ),
                enumeration!(
                    sensor.bayer,
                    BayerPhase,
                    "Bayer phase",
                    "Colour-filter phase error between sensor and demosaic."
                ),
                float!(
                    sensor.fpn,
                    "Pattern noise",
                    "Column fixed-pattern gain and per-frame row noise.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "retina",
            label: "Retina camera",
            description: "A Game Boy Camera sensor: tiny monochrome grid, edge enhancement, 4×4 dithered 2-bit output and a slow read clock.",
            settings: vec![
                enabled!(retina, "Retina camera"),
                int!(
                    retina.resolution,
                    "Resolution",
                    "Sensor rows across the picture height.",
                    16,
                    240
                ),
                enumeration!(
                    retina.edge_mode,
                    RetinaEdge,
                    "Edge mode",
                    "Edge-enhancement filter of the sensor."
                ),
                int!(
                    retina.edge,
                    "Edge ratio",
                    "Edge ratio register; α = 0.5, 0.75, 1, 1.25, 2, 3, 4, 5.",
                    0,
                    7
                ),
                int!(retina.levels, "Levels", "Output grey levels.", 2, 16),
                float!(
                    retina.dither,
                    "Dither",
                    "Strength of the 4×4 ordered-dither thresholds.",
                    0.0,
                    1.0
                ),
                float!(
                    retina.read_slow,
                    "Slow read",
                    "Slowed read clock: exposure grows down the frame.",
                    0.0,
                    1.0
                ),
                boolean!(retina.invert, "Invert", "Sensor invert bit."),
                enumeration!(retina.palette, RetinaPalette, "Palette", "Display palette."),
            ],
        },
        SettingGroup {
            id: "isp",
            label: "Camera ISP",
            description: "A camera DSP with bent control loops: exposure pumping, white balance hunting, over-driven aperture correction and gain noise.",
            settings: vec![
                enabled!(isp, "Camera ISP"),
                float!(
                    isp.hunt,
                    "Exposure hunt",
                    "Auto-exposure loop gain; above about 0.5 it overshoots and pumps.",
                    0.0,
                    1.0
                ),
                float!(
                    isp.white_balance,
                    "White balance",
                    "Auto-white-balance loop gain; high values swing colour casts.",
                    0.0,
                    1.0
                ),
                float!(
                    isp.sharpen,
                    "Sharpen",
                    "Aperture-correction gain: halos beside edges.",
                    0.0,
                    4.0
                ),
                int!(
                    isp.sharpen_radius,
                    "Sharpen radius",
                    "Halo width in pixels.",
                    1,
                    8
                ),
                float!(
                    isp.noise,
                    "Noise",
                    "Sensor noise, amplified with the exposure gain.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "tape",
            label: "Tape transport",
            description: "Worn VHS playback: head-switching skew, tracking noise, dropouts, soft FM luma, colour-under chroma and timebase wobble.",
            settings: vec![
                enabled!(tape, "Tape transport"),
                float!(
                    tape.head_switch,
                    "Head switch",
                    "Skewed, noisy lines at the bottom where the heads switch.",
                    0.0,
                    1.0
                ),
                float!(
                    tape.flagging,
                    "Flagging",
                    "Top-of-frame hook from tape tension.",
                    -1.0,
                    1.0
                ),
                float!(
                    tape.tracking,
                    "Tracking",
                    "Mistracking noise band drifting up the picture.",
                    0.0,
                    1.0
                ),
                float!(
                    tape.dropout,
                    "Dropout",
                    "Oxide dropouts: short white streaks.",
                    0.0,
                    1.0
                ),
                boolean!(
                    tape.doc,
                    "Dropout compensator",
                    "Repeat the line above instead of white."
                ),
                float!(
                    tape.comet,
                    "Comet",
                    "FM over-deviation: black streaks trailing highlights.",
                    0.0,
                    1.0
                ),
                float!(
                    tape.luma_bw,
                    "Luma softness",
                    "FM luma bandwidth loss (softness).",
                    0.0,
                    1.0
                ),
                float!(
                    tape.chroma_noise,
                    "Chroma noise",
                    "Colour-under smear and chroma noise.",
                    0.0,
                    1.0
                ),
                float!(
                    tape.jitter,
                    "Jitter",
                    "Capstan/drum timebase wobble.",
                    0.0,
                    1.0
                ),
            ],
        },
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
                    sync.jitter_smooth,
                    "Jitter smoothing",
                    "Row-to-row correlation of the jitter; high values make lines drift together like a hunting H-PLL.",
                    0.0,
                    0.99
                ),
                float!(
                    sync.tear_rate,
                    "Tear rate",
                    "Per-frame probability of a band of torn, brightened lines that relock as they go down.",
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
                boolean!(
                    sync.blanking,
                    "Blanking",
                    "Show the horizontal blanking interval as a black bar where shifted lines wrap."
                ),
                float!(
                    sync.clamp_error,
                    "Clamp error",
                    "Back-porch clamp samples active picture: each line's black level follows its left edge.",
                    0.0,
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
                float!(
                    clock.jitter,
                    "Jitter",
                    "Random sample-clock phase error per pixel, in pixels.",
                    0.0,
                    4.0
                ),
                float!(
                    clock.grid_drift,
                    "Grid drift",
                    "Sample-and-hold grid offset added per row, in pixels; staircases held blocks diagonally.",
                    0.0,
                    8.0
                ),
            ],
        },
        SettingGroup {
            id: "data",
            label: "Data bus",
            description: "Bridged ADC/RAM data lines: bits of each 8-bit sample are swapped, stuck or latched late.",
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
                    "Bridge bit A",
                    "First of two bridged data bits (equal bits = no bridge).",
                    0,
                    7
                ),
                int!(
                    data.swap_b,
                    "Bridge bit B",
                    "Second of two bridged data bits.",
                    0,
                    7
                ),
                enumeration!(
                    data.pair_mode,
                    PairMode,
                    "Bridge mode",
                    "How the two bridged bits interact."
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
                int!(
                    data.lag_bit,
                    "Lag bit",
                    "Bits at or above this index are latched late, from an earlier sample (8 = off).",
                    0,
                    8
                ),
                int!(
                    data.lag,
                    "Lag",
                    "How many pixels late the high bits are latched.",
                    1,
                    32
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
            id: "dct",
            label: "DCT codec",
            description: "An 8×8 DCT block codec with a bent quantizer ROM, scrambled coefficients, DC predictor errors and misplaced macroblocks.",
            settings: vec![
                enabled!(dct, "DCT codec"),
                int!(
                    dct.quality,
                    "Quality",
                    "JPEG-style quality; low values block and ring.",
                    1,
                    100
                ),
                int!(
                    dct.table_shift,
                    "Table shift",
                    "Quantizer ROM address offset: coefficients use the wrong step sizes.",
                    0,
                    63
                ),
                float!(
                    dct.scramble,
                    "Scramble",
                    "Per-block chance AC coefficients are read in a rotated zig-zag order.",
                    0.0,
                    1.0
                ),
                float!(
                    dct.dc_drift,
                    "DC drift",
                    "Per-block-row chance of a DC predictor error that offsets the rest of the row.",
                    0.0,
                    1.0
                ),
                float!(
                    dct.block_shuffle,
                    "Block shuffle",
                    "Per-macroblock chance of showing another macroblock from the same row.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "mosh",
            label: "Datamosh",
            description: "A decoder that lost its keyframes: block motion from the live picture drags the stale image around.",
            settings: vec![
                enabled!(mosh, "Datamosh"),
                float!(
                    mosh.keyframe_rate,
                    "Keyframe rate",
                    "Per-frame chance of a keyframe that refreshes the picture.",
                    0.0,
                    1.0
                ),
                int!(
                    mosh.search,
                    "Search range",
                    "Motion search range in pixels.",
                    0,
                    64
                ),
                float!(
                    mosh.vector_gain,
                    "Vector gain",
                    "Multiplier on motion vectors.",
                    -4.0,
                    4.0
                ),
                float!(
                    mosh.residual,
                    "Residual",
                    "How much of the P-frame residual is applied.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "hold",
            label: "Memory hold",
            description: "A glitched frame-store write-enable: bands, fields or whole frames keep the previous picture.",
            settings: vec![
                enabled!(hold, "Memory hold"),
                enumeration!(
                    hold.mode,
                    HoldMode,
                    "Mode",
                    "Which part of the store misses its write."
                ),
                float!(
                    hold.rate,
                    "Rate",
                    "Per-band (or per-field) probability it is not rewritten this frame.",
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
                int!(
                    hold.strobe_period,
                    "Strobe period",
                    "Strobe: frames between store writes.",
                    1,
                    60
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
        SettingGroup {
            id: "counter",
            label: "Address counters",
            description: "Bent frame-store counters: wrong line pitch shears the picture, early resets tile it, and a creeping start address scrolls it.",
            settings: vec![
                enabled!(counter, "Address counters"),
                int!(
                    counter.pitch,
                    "Pitch",
                    "Extra words per line: each row starts later, so the image shears and wraps.",
                    -64,
                    64
                ),
                float!(
                    counter.line_length,
                    "Line length",
                    "Column counter resets at this fraction of the width.",
                    0.05,
                    1.0
                ),
                float!(
                    counter.frame_length,
                    "Frame length",
                    "Row counter resets at this fraction of the height.",
                    0.05,
                    1.0
                ),
                float!(
                    counter.scroll,
                    "Scroll",
                    "Start address creeps each frame; 1 = 5% of the store per frame.",
                    -1.0,
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
            id: "colorize",
            label: "Colourizer",
            description: "A video-synth colourizer: luma is folded, sliced into bands and painted with hues, behind a luma key.",
            settings: vec![
                enabled!(colorize, "Colourizer"),
                enumeration!(
                    colorize.mode,
                    ColorizeMode,
                    "Mode",
                    "Flat colour bands or a continuous hue sweep."
                ),
                int!(colorize.bands, "Bands", "Number of luma bands.", 2, 16),
                int!(
                    colorize.fold,
                    "Fold",
                    "Frequency-doubler passes on luma before colouring.",
                    0,
                    4
                ),
                float!(
                    colorize.hue,
                    "Hue",
                    "Hue of the darkest band, in degrees.",
                    -180.0,
                    180.0
                ),
                float!(
                    colorize.spread,
                    "Spread",
                    "Hue turns across the full luma range.",
                    0.0,
                    4.0
                ),
                float!(
                    colorize.saturation,
                    "Saturation",
                    "Colour strength.",
                    0.0,
                    1.0
                ),
                float!(
                    colorize.key,
                    "Key",
                    "Pixels darker than this keep their original colour.",
                    0.0,
                    1.0
                ),
                float!(colorize.mix, "Mix", "Blend with the original.", 0.0, 1.0),
            ],
        },
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
            description: "A starved supply rail: bright lines dim and shrink, highlights streak, mains hum rolls and bends.",
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
                float!(
                    rail.hum_bend,
                    "Hum bend",
                    "Horizontal bending of lines by the hum, as from a ground loop in the deflection.",
                    0.0,
                    1.0
                ),
                float!(
                    rail.bars,
                    "Bars",
                    "Supply ripple at the pixel-fetch rate: fixed vertical bars.",
                    0.0,
                    1.0
                ),
                int!(
                    rail.bar_period,
                    "Bar period",
                    "Bar period in pixels.",
                    2,
                    16
                ),
            ],
        },
        SettingGroup {
            id: "osc",
            label: "Oscillation",
            description: "A self-oscillating op-amp: drifting diagonal interference bars and ringing after edges.",
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
                float!(
                    osc.ringing,
                    "Ringing",
                    "Near-unstable amplifier: damped ringing after edges at the oscillation frequency.",
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
                float!(
                    burst.hanover,
                    "Hanover",
                    "PAL-style alternate-line phase error: odd lines take the opposite hue error.",
                    0.0,
                    1.0
                ),
                float!(
                    burst.alternate_loss,
                    "Alternate loss",
                    "Encoder drops every other burst: odd lines lose colour.",
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
        SettingGroup {
            id: "composite",
            label: "Composite link",
            description: "The picture goes through a real NTSC composite signal and a cheap decoder: dot crawl, rainbows, cross-colour, ghosts and a detuned subcarrier.",
            settings: vec![
                enabled!(composite, "Composite link"),
                enumeration!(
                    composite.decoder,
                    Decoder,
                    "Decoder",
                    "Luma/chroma separation in the monitor."
                ),
                float!(
                    composite.trap,
                    "Chroma trap",
                    "Chroma-trap depth in luma; lower leaves the dot pattern in the picture.",
                    0.0,
                    1.0
                ),
                float!(
                    composite.detune,
                    "Detune",
                    "Decoder subcarrier detuning: hue rolls across and down the picture.",
                    -1.0,
                    1.0
                ),
                boolean!(
                    composite.dot_crawl,
                    "Dot crawl",
                    "Subcarrier phase flips each frame, so dots crawl."
                ),
                float!(
                    composite.peaking,
                    "Peaking",
                    "Luma peaking: outlines and ringing.",
                    0.0,
                    2.0
                ),
                float!(
                    composite.ghost,
                    "Ghost",
                    "Cable/antenna reflection: a delayed echo to the right.",
                    0.0,
                    1.0
                ),
                float!(composite.noise, "Noise", "RF snow.", 0.0, 1.0),
            ],
        },
        SettingGroup {
            id: "scan",
            label: "Scan processor",
            description: "Deflection driven by the picture: scanlines lift with brightness into a relief of lines, and an audio-rate wobble bends the raster.",
            settings: vec![
                enabled!(scan, "Scan processor"),
                float!(
                    scan.displace,
                    "Displace",
                    "Vertical lift of each scanline by its brightness; 1 = a quarter of the height.",
                    -1.0,
                    1.0
                ),
                int!(
                    scan.line_spacing,
                    "Line spacing",
                    "Draw every Nth source line.",
                    1,
                    16
                ),
                int!(
                    scan.thickness,
                    "Thickness",
                    "Drawn line thickness in rows.",
                    1,
                    4
                ),
                boolean!(
                    scan.ac_coupled,
                    "AC coupled",
                    "Lift by brightness relative to the line average (waveform only, no drift)."
                ),
                float!(scan.wobble, "Wobble", "Wobbulator depth.", 0.0, 1.0),
                float!(
                    scan.wobble_freq,
                    "Wobble frequency",
                    "Wobble cycles per pixel of beam time.",
                    0.0001,
                    0.05,
                    log
                ),
                float!(
                    scan.mix,
                    "Mix",
                    "1 = lines on black; lower shows the picture underneath.",
                    0.0,
                    1.0
                ),
            ],
        },
        SettingGroup {
            id: "feedback",
            label: "Feedback loop",
            description: "A camera re-shooting the monitor: the last output returns zoomed, rotated and hue-shifted, keyed into the live picture.",
            settings: vec![
                enabled!(feedback, "Feedback loop"),
                float!(
                    feedback.amount,
                    "Amount",
                    "How much of the fed-back picture is mixed in.",
                    0.0,
                    1.0
                ),
                float!(
                    feedback.zoom,
                    "Zoom",
                    "Scale per pass; above 1 tunnels outward.",
                    0.8,
                    1.25,
                    log
                ),
                float!(
                    feedback.rotate,
                    "Rotate",
                    "Rotation per pass in degrees.",
                    -10.0,
                    10.0
                ),
                float!(
                    feedback.offset_x,
                    "Offset X",
                    "Horizontal drift per pass, fraction of width.",
                    -0.1,
                    0.1
                ),
                float!(
                    feedback.offset_y,
                    "Offset Y",
                    "Vertical drift per pass, fraction of height.",
                    -0.1,
                    0.1
                ),
                float!(
                    feedback.hue_shift,
                    "Hue shift",
                    "Hue rotation per pass, in degrees.",
                    -180.0,
                    180.0
                ),
                float!(
                    feedback.gain,
                    "Gain",
                    "Loop brightness gain; above 1 blooms, below fades.",
                    0.5,
                    1.5
                ),
                float!(
                    feedback.blur,
                    "Blur",
                    "CRT/lens softness in the loop.",
                    0.0,
                    1.0
                ),
                float!(
                    feedback.key,
                    "Key",
                    "Feedback shows only where the live picture is darker than this (0 = everywhere).",
                    0.0,
                    1.0
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

    /// Whether stage `id` is switched into the chain.
    pub fn stage_enabled(&self, id: StageId) -> bool {
        match id {
            StageId::Sensor => self.sensor.enabled,
            StageId::Retina => self.retina.enabled,
            StageId::Isp => self.isp.enabled,
            StageId::Tape => self.tape.enabled,
            StageId::Sync => self.sync.enabled,
            StageId::Clock => self.clock.enabled,
            StageId::Data => self.data.enabled,
            StageId::Dct => self.dct.enabled,
            StageId::Mosh => self.mosh.enabled,
            StageId::Hold => self.hold.enabled,
            StageId::Counter => self.counter.enabled,
            StageId::AddressA => self.address_a.enabled,
            StageId::AddressB => self.address_b.enabled,
            StageId::Colorize => self.colorize.enabled,
            StageId::Invert => self.invert.enabled,
            StageId::Rail => self.rail.enabled,
            StageId::Osc => self.osc.enabled,
            StageId::Burst => self.burst.enabled,
            StageId::Bleed => self.bleed.enabled,
            StageId::Composite => self.composite.enabled,
            StageId::Scan => self.scan.enabled,
            StageId::Feedback => self.feedback.enabled,
        }
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
    /// JSON name. `"seed"` sets [`BendEffect::seed`]; `"order"` takes a
    /// comma-separated stage list (see [`ChainOrder::parse`]).
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
        if id == "order" {
            self.order = ChainOrder::parse(raw).map_err(|value| PresetError::InvalidValue {
                id: id.to_owned(),
                value,
            })?;
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
