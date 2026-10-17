//! Signal chain driver and per-stream state.

use crate::planes::{PixelOrder, YiqPlanes};
use crate::settings::BendEffect;
use crate::stages::{self, ADDRESS_A, ADDRESS_B, hold::HoldHistory};

/// Caller-owned per-stream state: working buffers and frame-store history.
///
/// Use one per video stream; call [`BendState::reset`] after a seek if desired
/// (discontinuities are also detected automatically from frame numbers).
#[derive(Default)]
pub struct BendState {
    planes: YiqPlanes,
    scratch: YiqPlanes,
    hold: HoldHistory,
}

impl BendState {
    /// Forgets the emulated frame-store memory.
    pub fn reset(&mut self) {
        self.hold = HoldHistory::default();
    }
}

impl BendEffect {
    /// Applies the effect in place to YIQ planes for frame `frame_num`.
    pub fn apply_yiq(&self, planes: &mut YiqPlanes, frame_num: u64, state: &mut BendState) {
        state.scratch.resize(planes.width, planes.height);
        run_chain(self, planes, &mut state.scratch, &mut state.hold, frame_num);
    }

    /// Applies the effect to packed 8-bit pixels from `src` into `dst`.
    ///
    /// # Panics
    /// If either buffer is too short for `height` rows of its stride.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_rgba8(
        &self,
        width: usize,
        height: usize,
        src: &[u8],
        src_stride: usize,
        dst: &mut [u8],
        dst_stride: usize,
        order: PixelOrder,
        frame_num: u64,
        state: &mut BendState,
    ) {
        state
            .planes
            .load_rgba8(width, height, src, src_stride, order);
        state.scratch.resize(width, height);
        run_chain(
            self,
            &mut state.planes,
            &mut state.scratch,
            &mut state.hold,
            frame_num,
        );
        state.planes.store_rgba8(dst, dst_stride, order);
    }
}

/// Input → TBC/sync → ADC → frame store → DAC → encoder.
fn run_chain(
    e: &BendEffect,
    planes: &mut YiqPlanes,
    scratch: &mut YiqPlanes,
    history: &mut HoldHistory,
    n: u64,
) {
    if planes.width == 0 || planes.height == 0 {
        return;
    }
    let seed = e.seed;
    if e.sync.enabled {
        stages::sync::apply(&e.sync.settings, seed, n, planes, scratch);
    }
    if e.clock.enabled {
        stages::clock::apply(&e.clock.settings, seed, n, planes, scratch);
    }
    if e.data.enabled {
        stages::data::apply(&e.data.settings, seed, n, planes);
    }
    if e.hold.enabled {
        stages::hold::apply(&e.hold.settings, seed, n, planes, history);
    } else {
        *history = HoldHistory::default();
    }
    if e.address_a.enabled {
        stages::address::apply(&e.address_a.settings, seed, ADDRESS_A, n, planes, scratch);
    }
    if e.address_b.enabled {
        stages::address::apply(&e.address_b.settings, seed, ADDRESS_B, n, planes, scratch);
    }
    if e.invert.enabled {
        stages::invert::apply(&e.invert.settings, planes);
    }
    if e.rail.enabled {
        stages::rail::apply(&e.rail.settings, n, planes, scratch);
    }
    if e.osc.enabled {
        stages::osc::apply(&e.osc.settings, n, planes);
    }
    if e.burst.enabled {
        stages::burst::apply(&e.burst.settings, seed, n, planes);
    }
    if e.bleed.enabled {
        stages::bleed::apply(&e.bleed.settings, planes, scratch);
    }
}
