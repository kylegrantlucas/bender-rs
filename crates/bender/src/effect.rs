//! Signal chain driver and per-stream state.

use crate::planes::{PixelOrder, YiqPlanes};
use crate::settings::{BendEffect, StageId};
use crate::stages::{self, memory::StageMemory};

/// Caller-owned per-stream state: working buffers and the memory of stateful
/// stages (frame store, sensor, ISP loops, datamosh references, feedback).
///
/// Use one per video stream; call [`BendState::reset`] after a seek if desired
/// (discontinuities are also detected automatically from frame numbers).
#[derive(Default)]
pub struct BendState {
    planes: YiqPlanes,
    work: Work,
}

/// Buffers shared by the stages of one stream.
#[derive(Default)]
struct Work {
    /// Gather target; stages that resample write here and swap it in.
    scratch: YiqPlanes,
    /// General-purpose float buffer; each stage resizes it as needed.
    buf: Vec<f32>,
    memory: StageMemory,
}

impl BendState {
    /// Forgets the memory of every stateful stage.
    pub fn reset(&mut self) {
        self.work.memory = StageMemory::default();
    }
}

impl BendEffect {
    /// Applies the effect in place to YIQ planes for frame `frame_num`.
    pub fn apply_yiq(&self, planes: &mut YiqPlanes, frame_num: u64, state: &mut BendState) {
        run_chain(self, planes, &mut state.work, frame_num);
    }

    /// Applies the effect to packed R, G, B bytes without an alpha channel.
    ///
    /// Strides are in bytes. Row padding is neither read nor written, and padding
    /// after the final row is optional. Empty frames do not access either buffer.
    /// Working planes and stage memory are reused through `state`.
    ///
    /// # Panics
    /// For nonempty frames, if either stride is less than `width * 3`, either
    /// buffer cannot hold every active row, or the required size overflows.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_rgb8(
        &self,
        width: usize,
        height: usize,
        input: &[u8],
        input_stride: usize,
        output: &mut [u8],
        output_stride: usize,
        frame_num: u64,
        state: &mut BendState,
    ) {
        if width != 0 && height != 0 {
            let row_bytes = width.checked_mul(3).expect("RGB row size overflow");
            for (len, stride) in [(input.len(), input_stride), (output.len(), output_stride)] {
                assert!(stride >= row_bytes, "RGB stride is shorter than a row");
                let required = (height - 1)
                    .checked_mul(stride)
                    .and_then(|offset| offset.checked_add(row_bytes))
                    .expect("RGB buffer size overflow");
                assert!(len >= required, "RGB buffer is too short");
            }
        }
        state.planes.load_rgb8(width, height, input, input_stride);
        run_chain(self, &mut state.planes, &mut state.work, frame_num);
        state.planes.store_rgb8(output, output_stride);
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
        run_chain(self, &mut state.planes, &mut state.work, frame_num);
        state.planes.store_rgba8(dst, dst_stride, order);
    }
}

/// Runs every enabled stage in [`BendEffect::order`].
fn run_chain(e: &BendEffect, planes: &mut YiqPlanes, work: &mut Work, n: u64) {
    if planes.width == 0 || planes.height == 0 {
        return;
    }
    work.scratch.resize(planes.width, planes.height);
    let seed = e.seed;
    let Work {
        scratch,
        buf,
        memory,
    } = work;
    for &id in e.order.stages() {
        if !e.stage_enabled(id) {
            continue;
        }
        match id {
            StageId::Sensor => stages::sensor::apply(
                &e.sensor.settings,
                seed,
                n,
                planes,
                scratch,
                buf,
                &mut memory.sensor,
            ),
            StageId::Retina => stages::retina::apply(&e.retina.settings, planes, buf),
            StageId::Isp => stages::isp::apply(
                &e.isp.settings,
                seed,
                n,
                planes,
                scratch,
                buf,
                &mut memory.isp,
            ),
            StageId::Tape => stages::tape::apply(&e.tape.settings, seed, n, planes, scratch, buf),
            StageId::Sync => stages::sync::apply(&e.sync.settings, seed, n, planes, scratch, buf),
            StageId::Clock => stages::clock::apply(&e.clock.settings, seed, n, planes, scratch),
            StageId::Data => stages::data::apply(&e.data.settings, seed, n, planes),
            StageId::Dct => stages::dct::apply(&e.dct.settings, seed, n, planes, scratch, buf),
            StageId::Mosh => {
                stages::mosh::apply(&e.mosh.settings, seed, n, planes, scratch, &mut memory.mosh)
            }
            StageId::Hold => {
                stages::hold::apply(&e.hold.settings, seed, n, planes, &mut memory.hold);
            }
            StageId::Counter => stages::counter::apply(&e.counter.settings, n, planes, scratch),
            StageId::AddressA => {
                stages::address::apply(&e.address_a.settings, seed, id, n, planes, scratch);
            }
            StageId::AddressB => {
                stages::address::apply(&e.address_b.settings, seed, id, n, planes, scratch);
            }
            StageId::Colorize => stages::colorize::apply(&e.colorize.settings, planes),
            StageId::Invert => stages::invert::apply(&e.invert.settings, planes),
            StageId::Rail => stages::rail::apply(&e.rail.settings, n, planes, scratch),
            StageId::Osc => stages::osc::apply(&e.osc.settings, n, planes),
            StageId::Burst => stages::burst::apply(&e.burst.settings, seed, n, planes),
            StageId::Bleed => stages::bleed::apply(&e.bleed.settings, planes, scratch),
            StageId::Composite => {
                stages::composite::apply(&e.composite.settings, seed, n, planes, buf);
            }
            StageId::Scan => stages::scan::apply(&e.scan.settings, n, planes, scratch),
            StageId::Feedback => stages::feedback::apply(
                &e.feedback.settings,
                n,
                planes,
                scratch,
                &mut memory.feedback,
            ),
        }
    }
    memory.forget_disabled(e);
}
