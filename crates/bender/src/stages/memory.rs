//! State that stateful stages carry between frames of one stream.

use crate::planes::YiqPlanes;
use crate::settings::BendEffect;

/// Two-slot state that follows frame numbers: re-rendering a frame sees the
/// same input state, consecutive frames hand state forward, and any other jump
/// starts from `T::default()`.
#[derive(Default)]
pub(crate) struct Continuity<T> {
    /// State entering the current frame.
    before: T,
    /// State leaving the current frame.
    after: T,
    frame: Option<u64>,
}

impl<T: Default> Continuity<T> {
    /// `n == last`: keep `before` (re-render). `n == last + 1`: hand `after`
    /// forward. Otherwise: `before = T::default()`.
    pub(crate) fn advance(&mut self, n: u64) {
        match self.frame {
            Some(last) if last == n => {}
            Some(last) if n > 0 && last == n - 1 => {
                std::mem::swap(&mut self.before, &mut self.after);
            }
            _ => self.before = T::default(),
        }
        self.frame = Some(n);
    }

    /// `(state entering frame n, slot for state leaving frame n)`.
    pub(crate) fn split(&mut self) -> (&T, &mut T) {
        (&self.before, &mut self.after)
    }
}

/// Exposure and white-balance loop state of the camera ISP.
#[derive(Clone, Copy, Debug)]
pub(crate) struct IspLoop {
    pub gain: f32,
    pub wb_i: f32,
    pub wb_q: f32,
}

impl Default for IspLoop {
    fn default() -> Self {
        Self {
            gain: 1.0,
            wb_i: 0.0,
            wb_q: 0.0,
        }
    }
}

/// Reference frames of the datamosh decoder.
#[derive(Default)]
pub(crate) struct MoshFrames {
    /// Live input of the previous frame (the motion-search reference).
    pub input: YiqPlanes,
    /// Output of the previous frame (what gets dragged around).
    pub output: YiqPlanes,
}

/// Per-stream memory of every stateful stage.
#[derive(Default)]
pub(crate) struct StageMemory {
    pub hold: Continuity<Option<YiqPlanes>>,
    pub sensor: Continuity<Option<YiqPlanes>>,
    pub isp: Continuity<IspLoop>,
    pub mosh: Continuity<Option<MoshFrames>>,
    pub feedback: Continuity<Option<YiqPlanes>>,
}

impl StageMemory {
    /// Forgets the state of every disabled stateful stage, so re-enabling one
    /// starts fresh instead of resuming stale memory.
    pub(crate) fn forget_disabled(&mut self, e: &BendEffect) {
        if !e.hold.enabled {
            self.hold = Continuity::default();
        }
        if !e.sensor.enabled {
            self.sensor = Continuity::default();
        }
        if !e.isp.enabled {
            self.isp = Continuity::default();
        }
        if !e.mosh.enabled {
            self.mosh = Continuity::default();
        }
        if !e.feedback.enabled {
            self.feedback = Continuity::default();
        }
    }
}

/// `stored` if it holds planes of exactly `w`×`h`.
pub(crate) fn matching(stored: &Option<YiqPlanes>, w: usize, h: usize) -> Option<&YiqPlanes> {
    stored.as_ref().filter(|p| p.width == w && p.height == h)
}

/// Copies `planes` into `slot`, reusing its allocation.
pub(crate) fn store(slot: &mut Option<YiqPlanes>, planes: &YiqPlanes) {
    slot.get_or_insert_with(YiqPlanes::default)
        .copy_from(planes);
}
